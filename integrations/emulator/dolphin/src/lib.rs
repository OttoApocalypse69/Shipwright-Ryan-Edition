//! External Dolphin runtime integration for Wii game images.
//!
//! SRE accepts a Dolphin executable selected by the user and passes through
//! the user's legally dumped game image. It never downloads or redistributes
//! Dolphin, game data, keys, firmware, or console material.

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

const RUNTIME_ID: &str = "dolphin-compatible";
const PLAYABLE_THRESHOLD_MS: u64 = 5_000;

pub struct DolphinRuntimeAdapter {
    id: RuntimeId,
    executable: Option<PathBuf>,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl DolphinRuntimeAdapter {
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            id: RuntimeId::new(RUNTIME_ID).expect("built-in runtime id is valid"),
            executable,
            children: Mutex::new(BTreeMap::new()),
        }
    }

    fn executable(&self) -> Result<&Path, RuntimeError> {
        self.executable
            .as_deref()
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeNotInstalled,
                    "Select a Dolphin-compatible runtime executable in SRE game setup.",
                )
            })
    }

    fn version(&self) -> Option<String> {
        let executable = self.executable.as_deref()?;
        let output = Command::new(executable)
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .ok()?;
        let bytes = if output.stdout.is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        let first = String::from_utf8_lossy(bytes)
            .lines()
            .next()?
            .trim()
            .to_owned();
        (!first.is_empty()).then_some(first)
    }

    fn source_supported(game: &GameDefinition, source: &GameSource) -> bool {
        game.variants.iter().any(|variant| {
            variant.id == source.variant_id
                && variant.original_platform == OriginalPlatform::Wii
                && variant
                    .runtime_candidates
                    .iter()
                    .any(|candidate| candidate.runtime_id.as_str() == RUNTIME_ID)
        })
    }

    fn supported_image(path: &Path) -> bool {
        path.is_file()
            && path.extension().is_some_and(|extension| {
                matches!(
                    extension.to_string_lossy().to_ascii_lowercase().as_str(),
                    "iso" | "wbfs" | "rvz" | "gcz" | "wia" | "ciso"
                )
            })
    }
}

impl RuntimeProvider for DolphinRuntimeAdapter {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    fn display_name(&self) -> &str {
        "Dolphin-compatible Wii runtime"
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::ExternalEmulator
    }

    fn distribution_mode(&self) -> DistributionMode {
        DistributionMode::External
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            native: false,
            external_process: true,
            supports_overlay: true,
            supports_semantic_events: false,
            supports_save_detection: true,
            supports_controller_config: true,
            supports_mods: true,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let Some(executable) = self.executable.as_ref().filter(|path| path.is_file()) else {
            return Ok(DetectionResult {
                runtime_id: self.id.clone(),
                status: DetectionStatus::Missing,
                installation: None,
                summary: "No Dolphin-compatible runtime executable is configured.".to_owned(),
            });
        };
        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: DetectionStatus::Available,
            installation: Some(RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: executable.parent().unwrap_or(Path::new(".")).to_owned(),
                executable: Some(executable.clone()),
                version: self
                    .version()
                    .or_else(|| Some("version unavailable".to_owned())),
            }),
            summary: "User-managed Dolphin-compatible runtime detected.".to_owned(),
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
                VersionStatus::Supported => "The selected Dolphin runtime reported a version.",
                VersionStatus::KnownBroken => {
                    "The selected Dolphin runtime is classified as known broken."
                }
                VersionStatus::Unknown => {
                    "The runtime did not report a version; compatibility remains experimental."
                }
                VersionStatus::Unsupported => "The Dolphin runtime version is unsupported.",
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
            && Self::supported_image(&source.path);
        Ok(SourceValidation {
            valid,
            detected_variant: valid.then(|| source.variant_id.clone()),
            summary: if valid {
                "A Wii game image compatible with Dolphin was found."
            } else {
                "Select a legal Wii game image (.iso, .wbfs, .rvz, .gcz, .wia, or .ciso)."
            }
            .to_owned(),
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
                "The Wii game image could not be validated for Dolphin.",
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
            && Self::supported_image(&installation.root);
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Dolphin Wii game image is registered."
            } else {
                "Registered Dolphin Wii game image is unavailable."
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
                "Register a valid Wii game image before launch.",
            ));
        }
        if config
            .values
            .keys()
            .any(|key| !["device_id", "fullscreen", "profile"].contains(&key.as_str()))
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Unsupported Dolphin runtime configuration key.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "The Dolphin adapter only launches authorized user sources.",
            ));
        }
        self.configure(&request.installation, &request.config)?;
        let executable = self.executable()?;
        let mut command = Command::new(executable);
        command
            .arg("-e")
            .arg(&request.installation.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command.spawn().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeLaunchFailed,
                "The Dolphin-compatible runtime could not be started.",
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
        let elapsed = session
            .started_at_unix_ms
            .map(|started| observed.observed_at_unix_ms.saturating_sub(started))
            .unwrap_or(0);
        Ok(PlayableDetection {
            playable: observed.running && elapsed >= PLAYABLE_THRESHOLD_MS,
            precision: PlayablePrecision::Approximate,
            method: format!("process alive plus {PLAYABLE_THRESHOLD_MS} ms startup threshold"),
            observed_at_unix_ms: observed.observed_at_unix_ms,
        })
    }

    fn find_save_location(
        &self,
        _installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        let path = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|root| root.join("Dolphin Emulator/Wii/title"));
        Ok(path.filter(|path| path.exists()).map(|path| SaveLocation {
            path,
            confidence: PlayablePrecision::Approximate,
            summary: "Dolphin's per-user Wii save root detected.".to_owned(),
        }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut output = vec![RuntimeDiagnostic {
            id: "dolphin-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available).then(|| {
                "Choose a Dolphin-compatible runtime executable in game setup.".to_owned()
            }),
        }];
        if let Some(installation) = installation {
            let result = self.verify(installation)?;
            output.push(RuntimeDiagnostic {
                id: "dolphin-game-source".to_owned(),
                severity: if result.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: result.summary,
                remediation: (!result.ready).then(|| {
                    "Select a legal Wii game image (.iso, .wbfs, .rvz, .gcz, .wia, or .ciso)."
                        .to_owned()
                }),
            });
        }
        output.push(RuntimeDiagnostic {
            id: "dolphin-playable-method".to_owned(),
            severity: DiagnosticSeverity::Warning,
            summary: "Playable detection is approximate for the external Dolphin runtime."
                .to_owned(),
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
    fn validates_a_user_supplied_skyward_sword_image() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("skyward-sword.rvz");
        fs::write(&image, b"fixture").unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let game = catalog
            .game(&GameId::new("zelda-skyward-sword").unwrap())
            .unwrap();
        let source = GameSource {
            variant_id: GameVariantId::new("wii").unwrap(),
            path: image,
            synthetic_fixture: false,
        };
        assert!(
            DolphinRuntimeAdapter::new(None)
                .validate_source(game, &source)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn rejects_non_wii_game_images() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("skyward-sword.nsp");
        fs::write(&image, b"fixture").unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let game = catalog
            .game(&GameId::new("zelda-skyward-sword").unwrap())
            .unwrap();
        let source = GameSource {
            variant_id: GameVariantId::new("wii").unwrap(),
            path: image,
            synthetic_fixture: false,
        };
        assert!(
            !DolphinRuntimeAdapter::new(None)
                .validate_source(game, &source)
                .unwrap()
                .valid
        );
    }
}
