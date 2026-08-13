//! SRE adapter for the Shipwright native runtime.

use serde::Deserialize;
use sre_core::{GameDefinition, RuntimeId};
use sre_runtime::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeInstallation, RuntimeKind,
    RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation, SourceValidation,
    VerificationResult, VersionStatus, VersionValidation,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

pub const SHIPWRIGHT_RUNTIME_PROTOCOL: u32 = 1;
const SHIPWRIGHT_RUNTIME_ID: &str = "shipwright";
const SHIPWRIGHT_PLATFORM: &str = "windows-x64";

#[derive(Debug, Clone, Deserialize)]
struct RuntimeMetadata {
    schema_version: u32,
    shipwright_version: String,
    runtime_protocol: u32,
    platform: String,
}

#[derive(Debug, Clone)]
struct RuntimeBundle {
    root: PathBuf,
    executable: PathBuf,
    resource_archive: PathBuf,
    metadata: RuntimeMetadata,
}

pub struct ShipwrightAdapter {
    id: RuntimeId,
    runtime_roots: Vec<PathBuf>,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl ShipwrightAdapter {
    pub fn new(runtime_roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            id: RuntimeId::new(SHIPWRIGHT_RUNTIME_ID)
                .expect("the built-in Shipwright runtime id must be valid"),
            runtime_roots: runtime_roots.into_iter().collect(),
            children: Mutex::new(BTreeMap::new()),
        }
    }

    pub const fn runtime_protocol(&self) -> u32 {
        SHIPWRIGHT_RUNTIME_PROTOCOL
    }

    fn inspect_runtime_root(&self, root: &Path) -> Result<Option<RuntimeBundle>, RuntimeError> {
        let executable = root.join("soh.exe");
        let resource_archive = root.join("soh.o2r");
        let metadata_path = root.join("sre-runtime.json");
        if !executable.is_file() || !resource_archive.is_file() || !metadata_path.is_file() {
            return Ok(None);
        }

        let bytes = fs::read(&metadata_path).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime metadata could not be read.",
            )
            .with_technical_details(error.to_string())
        })?;
        let metadata: RuntimeMetadata = serde_json::from_slice(&bytes).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime metadata is invalid.",
            )
            .with_technical_details(error.to_string())
        })?;
        if metadata.schema_version != 1
            || metadata.runtime_protocol != SHIPWRIGHT_RUNTIME_PROTOCOL
            || metadata.platform != SHIPWRIGHT_PLATFORM
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeVersionUnsupported,
                "The installed Shipwright runtime protocol or platform is incompatible.",
            )
            .with_technical_details(format!(
                "schema={}, protocol={}, platform={}",
                metadata.schema_version, metadata.runtime_protocol, metadata.platform
            )));
        }

        Ok(Some(RuntimeBundle {
            root: root.to_owned(),
            executable,
            resource_archive,
            metadata,
        }))
    }

    fn find_bundle(&self) -> Result<Option<RuntimeBundle>, RuntimeError> {
        for root in &self.runtime_roots {
            if let Some(bundle) = self.inspect_runtime_root(root)? {
                return Ok(Some(bundle));
            }
        }
        Ok(None)
    }

    fn copy_file_atomically(source: &Path, destination: &Path) -> Result<(), RuntimeError> {
        let parent = destination.parent().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource destination has no parent directory.",
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource directory could not be prepared.",
            )
            .with_technical_details(error.to_string())
        })?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".ftep-runtime-")
            .tempfile_in(parent)
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Runtime resource staging could not be created.",
                )
                .with_technical_details(error.to_string())
            })?;
        let mut input = fs::File::open(source).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource archive could not be opened.",
            )
            .with_technical_details(error.to_string())
        })?;
        io::copy(&mut input, temporary.as_file_mut())
            .and_then(|_| temporary.as_file_mut().flush())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Runtime resource archive could not be staged safely.",
                )
                .with_technical_details(error.to_string())
            })?;
        temporary.persist(destination).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource archive could not be promoted safely.",
            )
            .with_technical_details(error.error.to_string())
        })?;
        Ok(())
    }

    fn unsupported(operation: &str) -> RuntimeError {
        RuntimeError::new(
            RuntimeErrorCode::OperationUnsupported,
            format!(
                "Shipwright {operation} remains in the launcher during the incremental adapter migration."
            ),
        )
    }
}

impl RuntimeProvider for ShipwrightAdapter {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    fn display_name(&self) -> &str {
        "Ship of Harkinian"
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::NativePort
    }

    fn distribution_mode(&self) -> DistributionMode {
        DistributionMode::Bundled
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            native: true,
            external_process: true,
            supports_overlay: false,
            supports_semantic_events: false,
            supports_save_detection: false,
            supports_controller_config: false,
            supports_mods: false,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let Some(bundle) = self.find_bundle()? else {
            return Ok(DetectionResult {
                runtime_id: self.id.clone(),
                status: DetectionStatus::Missing,
                installation: None,
                summary: "The verified Shipwright runtime is not installed.".to_owned(),
            });
        };

        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: DetectionStatus::Available,
            installation: Some(RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: bundle.root,
                executable: Some(bundle.executable),
                version: Some(bundle.metadata.shipwright_version),
            }),
            summary: "Shipwright runtime located and protocol-compatible.".to_owned(),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        let current = self.find_bundle()?;
        let supported = current.as_ref().is_some_and(|bundle| {
            bundle.root == installation.root
                && installation.version.as_deref() == Some(&bundle.metadata.shipwright_version)
        });
        Ok(VersionValidation {
            status: if supported {
                VersionStatus::Supported
            } else {
                VersionStatus::Unsupported
            },
            detected_version: installation.version.clone(),
            summary: if supported {
                "Shipwright runtime protocol and platform are supported."
            } else {
                "Shipwright runtime metadata does not match the detected bundle."
            }
            .to_owned(),
        })
    }

    fn validate_source(
        &self,
        _game: &GameDefinition,
        _source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError> {
        Err(Self::unsupported("source validation"))
    }

    fn prepare(
        &self,
        _game: &GameDefinition,
        _source: &GameSource,
        _context: &PreparationContext,
    ) -> Result<PreparedGame, RuntimeError> {
        Err(Self::unsupported("asset preparation"))
    }

    fn verify(&self, installation: &GameInstallation) -> Result<VerificationResult, RuntimeError> {
        let provider_matches = installation.runtime_id == self.id;
        let archive_present = installation.root.join("oot.o2r").is_file()
            || installation.root.join("oot-mq.o2r").is_file();
        let ready = provider_matches && !installation.synthetic_fixture && archive_present;
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Imported OoT assets are ready for Shipwright."
            } else {
                "The Shipwright installation is missing a verified imported OoT archive."
            }
            .to_owned(),
        })
    }

    fn configure(
        &self,
        installation: &GameInstallation,
        _config: &RuntimeConfig,
    ) -> Result<(), RuntimeError> {
        if !self.verify(installation)?.ready {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameNotConfigured,
                "Shipwright game assets are not ready.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "Shipwright refuses synthetic fixture launches.",
            ));
        }
        if request.session_id.trim().is_empty() {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "A non-empty FTEP session id is required.",
            ));
        }
        self.configure(&request.installation, &request.config)?;

        let bundle = self.find_bundle()?.ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeNotInstalled,
                "The verified Shipwright runtime is not installed.",
            )
        })?;
        Self::copy_file_atomically(
            &bundle.resource_archive,
            &request.installation.root.join("soh.o2r"),
        )?;

        let child = Command::new(&bundle.executable)
            .current_dir(&request.installation.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeLaunchFailed,
                    "Shipwright could not be started.",
                )
                .with_technical_details(error.to_string())
            })?;

        let process_id = child.id();
        self.children
            .lock()
            .map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::Internal,
                    "Runtime process registry is unavailable.",
                )
            })?
            .insert(process_id, child);

        Ok(RuntimeSession {
            session_id: request.session_id,
            game_id: request.installation.game_id,
            variant_id: request.installation.variant_id,
            runtime_id: self.id.clone(),
            device_id: request.config.values.get("device_id").cloned(),
            state: RuntimeSessionState::RuntimeStarted,
            process_id: Some(process_id),
            synthetic_fixture: false,
            requested_at_unix_ms: now_unix_ms(),
            started_at_unix_ms: Some(now_unix_ms()),
            playable_at_unix_ms: None,
            ended_at_unix_ms: None,
            duration_ms: None,
            exit_code: None,
            launch_result: "PROCESS_STARTED".to_owned(),
            initial_events: vec![],
        })
    }

    fn observe_process(
        &self,
        session: &RuntimeSession,
    ) -> Result<ProcessObservation, RuntimeError> {
        let Some(process_id) = session.process_id else {
            return Ok(ProcessObservation {
                running: false,
                observed_at_unix_ms: now_unix_ms(),
                exit_code: session.exit_code,
            });
        };
        let mut children = self.children.lock().map_err(|_| {
            RuntimeError::new(
                RuntimeErrorCode::Internal,
                "Runtime process registry is unavailable.",
            )
        })?;
        let Some(child) = children.get_mut(&process_id) else {
            return Ok(ProcessObservation {
                running: session.exit_code.is_none(),
                observed_at_unix_ms: now_unix_ms(),
                exit_code: session.exit_code,
            });
        };
        let exit = child.try_wait().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::Internal,
                "Shipwright process state could not be observed.",
            )
            .with_technical_details(error.to_string())
        })?;
        Ok(ProcessObservation {
            running: exit.is_none(),
            observed_at_unix_ms: now_unix_ms(),
            exit_code: exit.and_then(|status| status.code()),
        })
    }

    fn determine_playable_state(
        &self,
        session: &RuntimeSession,
    ) -> Result<PlayableDetection, RuntimeError> {
        let observation = self.observe_process(session)?;
        let elapsed = session
            .started_at_unix_ms
            .map(|started| observation.observed_at_unix_ms.saturating_sub(started))
            .unwrap_or(0);
        Ok(PlayableDetection {
            playable: observation.running && elapsed >= 3_000,
            precision: PlayablePrecision::Approximate,
            method: "process alive plus 3 second startup threshold".to_owned(),
            observed_at_unix_ms: observation.observed_at_unix_ms,
        })
    }

    fn find_save_location(
        &self,
        installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        let path = installation.root.join("Save");
        Ok(path.exists().then_some(SaveLocation {
            path,
            confidence: PlayablePrecision::Approximate,
            summary: "Shipwright save directory detected beside the imported assets.".to_owned(),
        }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut findings = vec![RuntimeDiagnostic {
            id: "shipwright-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available)
                .then(|| "Repair or reinstall the SRE-bundled native runtime.".to_owned()),
        }];
        if let Some(installation) = installation {
            let verification = self.verify(installation)?;
            findings.push(RuntimeDiagnostic {
                id: "shipwright-game".to_owned(),
                severity: if verification.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: verification.summary,
                remediation: (!verification.ready)
                    .then(|| "Re-import legally supplied compatible game data.".to_owned()),
            });
        }
        Ok(findings)
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sre_core::{GameId, GameVariantId};
    use sre_runtime::{LaunchMode, RuntimeConfig};

    fn write_bundle(root: &Path, protocol: u32) {
        fs::write(root.join("soh.exe"), b"exe").unwrap();
        fs::write(root.join("soh.o2r"), b"resource").unwrap();
        fs::write(
            root.join("sre-runtime.json"),
            format!(
                r#"{{"schema_version":1,"shipwright_version":"9.2.3","runtime_protocol":{protocol},"platform":"windows-x64"}}"#
            ),
        )
        .unwrap();
    }

    fn fixture_installation(root: PathBuf) -> GameInstallation {
        GameInstallation {
            installation_id: "fixture-installation".to_owned(),
            game_id: GameId::new("zelda-oot").unwrap(),
            variant_id: GameVariantId::new("n64").unwrap(),
            runtime_id: RuntimeId::new("shipwright").unwrap(),
            root,
            synthetic_fixture: false,
        }
    }

    #[test]
    fn detects_a_protocol_compatible_runtime() {
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), SHIPWRIGHT_RUNTIME_PROTOCOL);
        let adapter = ShipwrightAdapter::new([directory.path().to_owned()]);
        let detection = adapter.detect().unwrap();
        assert_eq!(detection.status, DetectionStatus::Available);
        assert_eq!(
            detection.installation.unwrap().version.as_deref(),
            Some("9.2.3")
        );
    }

    #[test]
    fn rejects_incompatible_runtime_metadata() {
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), SHIPWRIGHT_RUNTIME_PROTOCOL + 1);
        let adapter = ShipwrightAdapter::new([directory.path().to_owned()]);
        assert_eq!(
            adapter.detect().unwrap_err().code,
            RuntimeErrorCode::RuntimeVersionUnsupported
        );
    }

    #[test]
    fn verification_requires_real_imported_oot_assets() {
        let directory = tempfile::tempdir().unwrap();
        let adapter = ShipwrightAdapter::new([]);
        let installation = fixture_installation(directory.path().to_owned());
        assert!(!adapter.verify(&installation).unwrap().ready);
        fs::write(directory.path().join("oot.o2r"), b"assets").unwrap();
        assert!(adapter.verify(&installation).unwrap().ready);
    }

    #[test]
    fn atomic_resource_copy_preserves_content() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.o2r");
        let destination = directory.path().join("destination.o2r");
        fs::write(&destination, b"same-len").unwrap();
        fs::write(&source, b"resource").unwrap();
        ShipwrightAdapter::copy_file_atomically(&source, &destination).unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"resource");
    }

    #[test]
    fn refuses_synthetic_launch_before_process_creation() {
        let adapter = ShipwrightAdapter::new([]);
        let mut installation = fixture_installation(PathBuf::from("fixture"));
        installation.synthetic_fixture = true;
        let error = adapter
            .launch(LaunchRequest {
                session_id: "session-1".to_owned(),
                installation,
                config: RuntimeConfig::default(),
                mode: LaunchMode::SyntheticFixture,
            })
            .unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::SyntheticLaunchForbidden);
    }
}
