//! Generic external Switch runtime provider. Concrete implementations are
//! user-configured descriptions; games depend only on the `switch-runtime` id.

use sre_core::{GameDefinition, OriginalPlatform, RuntimeId};
use sre_runtime::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeInstallation, RuntimeKind,
    RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation, SourceValidation,
    VerificationResult, VersionStatus, VersionValidation,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

const RUNTIME_ID: &str = "switch-runtime";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalSwitchImplementation {
    pub implementation_id: String,
    pub display_name: String,
    pub executable: PathBuf,
    pub version_arguments: Vec<String>,
    pub launch_arguments: Vec<String>,
    pub save_root: Option<PathBuf>,
    pub playable_threshold_ms: u64,
}

impl ExternalSwitchImplementation {
    pub fn manually_configured(
        implementation_id: impl Into<String>,
        display_name: impl Into<String>,
        executable: PathBuf,
    ) -> Self {
        Self {
            implementation_id: implementation_id.into(),
            display_name: display_name.into(),
            executable,
            version_arguments: vec!["--version".to_owned()],
            // Ryujinx/Ryubing builds may still attempt the retired GitHub
            // release endpoint even when their persisted setting says not to
            // check. Keep managed game launches offline and avoid showing an
            // update-check error before the game window opens.
            launch_arguments: vec!["--hide-updates".to_owned()],
            save_root: None,
            playable_threshold_ms: 5_000,
        }
    }
}

pub struct SwitchRuntimeProvider {
    id: RuntimeId,
    implementation: Option<ExternalSwitchImplementation>,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl SwitchRuntimeProvider {
    pub fn new(implementation: Option<ExternalSwitchImplementation>) -> Self {
        Self {
            id: RuntimeId::new(RUNTIME_ID).expect("built-in runtime id is valid"),
            implementation,
            children: Mutex::new(BTreeMap::new()),
        }
    }

    fn implementation(&self) -> Result<&ExternalSwitchImplementation, RuntimeError> {
        self.implementation
            .as_ref()
            .filter(|value| value.executable.is_file())
            .ok_or_else(|| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeNotInstalled,
                    "Select an external Switch runtime executable in SRE setup.",
                )
            })
    }

    fn source_supported(game: &GameDefinition, source: &GameSource) -> bool {
        game.variants.iter().any(|variant| {
            variant.id == source.variant_id
                && variant.original_platform == OriginalPlatform::Switch
                && variant
                    .runtime_candidates
                    .iter()
                    .any(|candidate| candidate.runtime_id.as_str() == RUNTIME_ID)
        })
    }

    fn version(&self) -> Option<String> {
        let implementation = self.implementation.as_ref()?;
        let output = Command::new(&implementation.executable)
            .args(&implementation.version_arguments)
            .stdin(Stdio::null())
            .output()
            .ok()?;
        let bytes = if output.stdout.is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        let value = String::from_utf8_lossy(bytes);
        let first = value.lines().next()?.trim();
        (!first.is_empty()).then(|| first.to_owned())
    }
}

impl RuntimeProvider for SwitchRuntimeProvider {
    fn id(&self) -> &RuntimeId {
        &self.id
    }
    fn display_name(&self) -> &str {
        "External Switch runtime"
    }
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::ExternalRuntime
    }
    fn distribution_mode(&self) -> DistributionMode {
        DistributionMode::ManualOnly
    }
    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            native: false,
            external_process: true,
            supports_overlay: true,
            supports_semantic_events: false,
            supports_save_detection: self
                .implementation
                .as_ref()
                .is_some_and(|value| value.save_root.is_some()),
            supports_controller_config: true,
            supports_mods: false,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let Some(implementation) = self
            .implementation
            .as_ref()
            .filter(|value| value.executable.is_file())
        else {
            return Ok(DetectionResult {
                runtime_id: self.id.clone(),
                status: DetectionStatus::Missing,
                installation: None,
                summary: "No external Switch runtime implementation is configured.".to_owned(),
            });
        };
        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: DetectionStatus::Available,
            installation: Some(RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: implementation
                    .executable
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_owned(),
                executable: Some(implementation.executable.clone()),
                version: self
                    .version()
                    .or_else(|| Some("version unavailable".to_owned())),
            }),
            summary: format!(
                "{} is configured as the external Switch implementation.",
                implementation.display_name
            ),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        let status = match installation.version.as_deref() {
            Some(value) if value.to_ascii_lowercase().contains("broken") => {
                VersionStatus::KnownBroken
            }
            Some("version unavailable") | None => VersionStatus::Unknown,
            Some(_) => VersionStatus::Supported,
        };
        Ok(VersionValidation {
            status,
            detected_version: installation.version.clone(),
            summary: match status {
                VersionStatus::Supported => "The external Switch runtime reported a version.",
                VersionStatus::KnownBroken => {
                    "This configured runtime version is classified as broken."
                }
                VersionStatus::Unknown => {
                    "Version reporting is unavailable; compatibility remains experimental."
                }
                VersionStatus::Unsupported => "This runtime version is unsupported.",
            }
            .to_owned(),
        })
    }

    fn validate_source(
        &self,
        game: &GameDefinition,
        source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError> {
        let valid = Self::source_supported(game, source)
            && !source.synthetic_fixture
            && source.path.is_file();
        Ok(SourceValidation {
            valid,
            detected_variant: valid.then(|| source.variant_id.clone()),
            summary: if valid {
                "The user-selected Switch game file exists and the catalog supports this variant."
            } else {
                "Select a legal game file for a cataloged Switch variant. SRE performs no decryption or acquisition."
            }.to_owned(),
        })
    }

    fn prepare(
        &self,
        game: &GameDefinition,
        source: &GameSource,
        _context: &PreparationContext,
    ) -> Result<PreparedGame, RuntimeError> {
        if !self.validate_source(game, source)?.valid {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "The Switch game source could not be registered.",
            ));
        }
        Ok(PreparedGame {
            game_id: game.id.clone(),
            variant_id: source.variant_id.clone(),
            runtime_id: self.id.clone(),
            prepared_root: source.path.clone(),
            synthetic_fixture: false,
        })
    }

    fn verify(&self, installation: &GameInstallation) -> Result<VerificationResult, RuntimeError> {
        let ready = installation.runtime_id == self.id
            && !installation.synthetic_fixture
            && installation.root.is_file();
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Switch game source is registered."
            } else {
                "Registered Switch game source is unavailable."
            }
            .to_owned(),
        })
    }

    fn configure(
        &self,
        installation: &GameInstallation,
        config: &RuntimeConfig,
    ) -> Result<(), RuntimeError> {
        if !self.verify(installation)?.ready {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameNotConfigured,
                "Register a valid Switch game source before launch.",
            ));
        }
        if config
            .values
            .keys()
            .any(|key| !["device_id", "fullscreen", "profile"].contains(&key.as_str()))
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Unsupported Switch runtime configuration key.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "The external Switch adapter only launches authorized user sources.",
            ));
        }
        self.configure(&request.installation, &request.config)?;
        let implementation = self.implementation()?;
        let mut command = Command::new(&implementation.executable);
        command
            .args(&implementation.launch_arguments)
            .arg(&request.installation.root);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command.spawn().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeLaunchFailed,
                "The external Switch runtime could not be started.",
            )
            .with_technical_details(error.to_string())
        })?;
        let pid = child.id();
        self.children
            .lock()
            .map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::Internal,
                    "Runtime process registry is unavailable.",
                )
            })?
            .insert(pid, child);
        let now = now_unix_ms();
        Ok(RuntimeSession {
            session_id: request.session_id,
            game_id: request.installation.game_id,
            variant_id: request.installation.variant_id,
            runtime_id: self.id.clone(),
            device_id: request.config.values.get("device_id").cloned(),
            state: RuntimeSessionState::RuntimeStarted,
            process_id: Some(pid),
            synthetic_fixture: false,
            requested_at_unix_ms: now,
            started_at_unix_ms: Some(now),
            playable_at_unix_ms: None,
            ended_at_unix_ms: None,
            duration_ms: None,
            exit_code: None,
            launch_result: "PROCESS_STARTED".to_owned(),
            initial_events: vec![sre_runtime::RuntimeEvent::GameBooted { at_unix_ms: now }],
        })
    }

    fn observe_process(
        &self,
        session: &RuntimeSession,
    ) -> Result<ProcessObservation, RuntimeError> {
        let Some(pid) = session.process_id else {
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
        let Some(child) = children.get_mut(&pid) else {
            return Ok(ProcessObservation {
                running: session.exit_code.is_none(),
                observed_at_unix_ms: now_unix_ms(),
                exit_code: session.exit_code,
            });
        };
        let exit = child.try_wait().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::Internal,
                "Runtime process state could not be observed.",
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
        let observed = self.observe_process(session)?;
        let threshold = self
            .implementation
            .as_ref()
            .map(|value| value.playable_threshold_ms)
            .unwrap_or(5_000);
        let elapsed = session
            .started_at_unix_ms
            .map(|started| observed.observed_at_unix_ms.saturating_sub(started))
            .unwrap_or(0);
        Ok(PlayableDetection {
            playable: observed.running && elapsed >= threshold,
            precision: PlayablePrecision::Approximate,
            method: format!("process alive plus {threshold} ms configured startup threshold"),
            observed_at_unix_ms: observed.observed_at_unix_ms,
        })
    }

    fn find_save_location(
        &self,
        _installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        Ok(self
            .implementation
            .as_ref()
            .and_then(|value| value.save_root.clone())
            .filter(|path| path.exists())
            .map(|path| SaveLocation {
                path,
                confidence: PlayablePrecision::Approximate,
                summary: "User-configured external runtime save root.".to_owned(),
            }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut output = vec![RuntimeDiagnostic {
            id: "switch-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available)
                .then(|| "Choose an external Switch runtime executable in SRE setup.".to_owned()),
        }];
        if let Some(installation) = installation {
            let result = self.verify(installation)?;
            output.push(RuntimeDiagnostic {
                id: "switch-game-source".to_owned(),
                severity: if result.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: result.summary,
                remediation: (!result.ready)
                    .then(|| "Select the legal game file again; it may have moved.".to_owned()),
            });
        }
        output.push(RuntimeDiagnostic {
            id: "switch-playable-method".to_owned(),
            severity: DiagnosticSeverity::Warning,
            summary: "Playable detection is approximate for external Switch runtimes.".to_owned(),
            remediation: None,
        });
        Ok(output)
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sre_core::{GameCatalog, GameId, GameVariantId};
    use std::fs;

    const CATALOG: &str = include_str!("../../../../packages/game-catalog/catalog.v1.json");

    #[test]
    fn one_provider_validates_all_three_switch_games() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("user-game.nsp");
        fs::write(&source_path, b"fixture").unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let provider = SwitchRuntimeProvider::new(None);
        for id in [
            "zelda-totk",
            "zelda-echoes-of-wisdom",
            "animal-crossing-new-horizons",
        ] {
            let game = catalog.game(&GameId::new(id).unwrap()).unwrap();
            let source = GameSource {
                variant_id: GameVariantId::new("switch").unwrap(),
                path: source_path.clone(),
                synthetic_fixture: false,
            };
            assert!(
                provider.validate_source(game, &source).unwrap().valid,
                "{id}"
            );
        }
    }

    #[test]
    fn rejects_non_switch_games_without_game_specific_branches() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("user-game.nsp");
        fs::write(&source_path, b"fixture").unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let source = GameSource {
            variant_id: GameVariantId::new("n64").unwrap(),
            path: source_path,
            synthetic_fixture: false,
        };
        let game = catalog.game(&GameId::new("zelda-oot").unwrap()).unwrap();
        assert!(
            !SwitchRuntimeProvider::new(None)
                .validate_source(game, &source)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn managed_launch_suppresses_retired_update_endpoint() {
        let implementation = ExternalSwitchImplementation::manually_configured(
            "managed-ryujinx-canary",
            "Managed Ryujinx Canary",
            PathBuf::from("Ryujinx.exe"),
        );
        assert_eq!(implementation.launch_arguments, vec!["--hide-updates"]);
    }
}
