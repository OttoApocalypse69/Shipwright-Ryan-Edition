//! Managed adapter for the official 2 Ship 2 Harkinian Windows runtime.
//!
//! The runtime itself performs the first-run conversion from a user-supplied
//! Majora's Mask ROM into its own generated archive.  SRE never copies or
//! retains that ROM: it passes its original path to the runtime on launch.

use sre_core::{GameDefinition, OriginalPlatform, RuntimeId};
use sre_runtime::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeInstallation,
    RuntimeKind, RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation,
    SourceValidation, VerificationResult, VersionStatus, VersionValidation,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

pub const RUNTIME_ID: &str = "two-ship";

pub struct TwoShipAdapter {
    id: RuntimeId,
    executable: PathBuf,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl TwoShipAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            id: RuntimeId::new(RUNTIME_ID).expect("the built-in Two Ship id is valid"),
            executable,
            children: Mutex::new(BTreeMap::new()),
        }
    }

    fn executable(&self) -> Result<&Path, RuntimeError> {
        self.executable
            .is_file()
            .then_some(self.executable.as_path())
            .ok_or_else(|| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeNotInstalled,
                    "SRE's bundled Majora's Mask runtime is missing. Reinstall SRE.",
                )
            })
    }

    fn is_n64_rom(path: &Path) -> bool {
        path.is_file()
            && path.extension().is_some_and(|extension| {
                matches!(
                    extension.to_string_lossy().to_ascii_lowercase().as_str(),
                    "z64" | "n64" | "v64"
                )
            })
    }

    fn source_supported(game: &GameDefinition, source: &GameSource) -> bool {
        game.variants.iter().any(|variant| {
            variant.id == source.variant_id
                && variant.original_platform == OriginalPlatform::Nintendo64
                && variant
                    .runtime_candidates
                    .iter()
                    .any(|candidate| candidate.runtime_id.as_str() == RUNTIME_ID)
        })
    }
}

impl RuntimeProvider for TwoShipAdapter {
    fn id(&self) -> &RuntimeId {
        &self.id
    }
    fn display_name(&self) -> &str {
        "2 Ship 2 Harkinian"
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
            supports_overlay: true,
            supports_semantic_events: false,
            supports_save_detection: true,
            supports_controller_config: true,
            supports_mods: true,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let installed = self.executable().is_ok();
        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: if installed {
                DetectionStatus::Available
            } else {
                DetectionStatus::Missing
            },
            installation: installed.then(|| RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: self
                    .executable
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_owned(),
                executable: Some(self.executable.clone()),
                version: Some("5.0.0".to_owned()),
            }),
            summary: if installed {
                "Bundled 2 Ship 2 Harkinian runtime is ready."
            } else {
                "Bundled 2 Ship 2 Harkinian runtime is missing."
            }
            .to_owned(),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        Ok(VersionValidation {
            status: if installation
                .executable
                .as_ref()
                .is_some_and(|path| path.is_file())
            {
                VersionStatus::Supported
            } else {
                VersionStatus::Unsupported
            },
            detected_version: installation.version.clone(),
            summary: "SRE bundles the reviewed 2 Ship 2 Harkinian 5.0.0 Windows runtime."
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
            && Self::is_n64_rom(&source.path);
        Ok(SourceValidation { valid, detected_variant: valid.then(|| source.variant_id.clone()), summary: if valid { "Majora's Mask game data selected. 2 Ship will verify and create its generated archive on first launch." } else { "Choose a legally acquired Majora's Mask .z64, .n64, or .v64 game image." }.to_owned() })
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
                "The selected Majora's Mask game image is not ready.",
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
            && Self::is_n64_rom(&installation.root)
            && self.executable().is_ok();
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Majora's Mask is ready for the bundled runtime."
            } else {
                "The bundled runtime or selected Majora's Mask game image is missing."
            }
            .to_owned(),
        })
    }

    fn configure(
        &self,
        installation: &GameInstallation,
        _config: &RuntimeConfig,
    ) -> Result<(), RuntimeError> {
        if self.verify(installation)?.ready {
            Ok(())
        } else {
            Err(RuntimeError::new(
                RuntimeErrorCode::GameNotConfigured,
                "Choose a valid Majora's Mask game image in SRE setup.",
            ))
        }
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "2 Ship launches only authorized user-supplied game data.",
            ));
        }
        self.configure(&request.installation, &request.config)?;
        let executable = self.executable()?;
        let root = executable.parent().unwrap_or(Path::new("."));
        let child = Command::new(executable)
            .arg(&request.installation.root)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeLaunchFailed,
                    "2 Ship 2 Harkinian could not be started.",
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
            initial_events: vec![RuntimeEvent::GameBooted { at_unix_ms: now }],
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
        _installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        let path = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|root| root.join("2Ship2Harkinian"));
        Ok(path
            .filter(|value| value.exists())
            .map(|path| SaveLocation {
                path,
                confidence: PlayablePrecision::Approximate,
                summary: "2 Ship runtime data directory detected.".to_owned(),
            }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut diagnostics = vec![RuntimeDiagnostic {
            id: "two-ship-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available)
                .then(|| "Reinstall SRE to restore the bundled Majora's Mask runtime.".to_owned()),
        }];
        if let Some(installation) = installation {
            let verification = self.verify(installation)?;
            diagnostics.push(RuntimeDiagnostic {
                id: "two-ship-game-source".to_owned(),
                severity: if verification.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: verification.summary,
                remediation: (!verification.ready)
                    .then(|| "Select a legally acquired Majora's Mask N64 game image.".to_owned()),
            });
        }
        Ok(diagnostics)
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
    #[test]
    fn accepts_only_standard_n64_rom_extensions() {
        let root = std::env::temp_dir().join(format!("sre-two-ship-test-{}", std::process::id()));
        std::fs::write(&root, b"fixture").unwrap();
        assert!(!TwoShipAdapter::is_n64_rom(&root.with_extension("z64")));
        let rom = root.with_extension("z64");
        std::fs::rename(&root, &rom).unwrap();
        assert!(TwoShipAdapter::is_n64_rom(&rom));
        assert!(!TwoShipAdapter::is_n64_rom(&rom.with_extension("txt")));
        std::fs::remove_file(rom).unwrap();
    }
}
