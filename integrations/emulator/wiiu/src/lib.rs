//! External Wii U runtime integration. This adapter never downloads a runtime,
//! console material, or game content.

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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

const RUNTIME_ID: &str = "cemu-compatible";

pub struct WiiURuntimeAdapter {
    id: RuntimeId,
    executable: Option<PathBuf>,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl WiiURuntimeAdapter {
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            id: RuntimeId::new(RUNTIME_ID).expect("built-in runtime id is valid"),
            executable,
            children: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn with_detected_windows_paths(manual: Option<PathBuf>) -> Self {
        if manual.as_ref().is_some_and(|path| path.is_file()) {
            return Self::new(manual);
        }
        let candidates = [
            std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .map(|p| p.join("Cemu/Cemu.exe")),
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .map(|p| p.join("Cemu/Cemu.exe")),
        ];
        Self::new(candidates.into_iter().flatten().find(|path| path.is_file()))
    }

    fn executable(&self) -> Result<&Path, RuntimeError> {
        self.executable
            .as_deref()
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeNotInstalled,
                    "Select a Cemu-compatible runtime executable in SRE game setup.",
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
        let value = if output.stdout.is_empty() {
            String::from_utf8_lossy(&output.stderr)
        } else {
            String::from_utf8_lossy(&output.stdout)
        };
        let value = value.lines().next()?.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }

    fn launch_target(root: &Path) -> Option<PathBuf> {
        if root.is_file() {
            return root
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rpx"))
                .then(|| root.to_owned());
        }
        let code = root.join("code");
        fs::read_dir(code)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("rpx"))
            })
    }
}

impl RuntimeProvider for WiiURuntimeAdapter {
    fn id(&self) -> &RuntimeId {
        &self.id
    }
    fn display_name(&self) -> &str {
        "Cemu-compatible Wii U runtime"
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
                summary: "No Cemu-compatible runtime executable is configured.".to_owned(),
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
            summary: "User-managed Cemu-compatible runtime detected.".to_owned(),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        let version = installation.version.clone();
        let status = match version.as_deref() {
            Some(value) if value.to_ascii_lowercase().contains("broken") => {
                VersionStatus::KnownBroken
            }
            Some("version unavailable") | None => VersionStatus::Unknown,
            Some(_) => VersionStatus::Supported,
        };
        Ok(VersionValidation {
            status,
            detected_version: version,
            summary: match status {
                VersionStatus::Supported => "The selected external runtime reported a version.",
                VersionStatus::KnownBroken => "The selected runtime is classified as known broken.",
                VersionStatus::Unknown => {
                    "The runtime did not report a version; compatibility remains experimental."
                }
                VersionStatus::Unsupported => "The runtime version is unsupported.",
            }
            .to_owned(),
        })
    }

    fn validate_source(
        &self,
        game: &GameDefinition,
        source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError> {
        let supports_runtime = game.variants.iter().any(|variant| {
            variant.id == source.variant_id
                && variant
                    .runtime_candidates
                    .iter()
                    .any(|candidate| candidate.runtime_id == self.id)
        });
        let valid = supports_runtime
            && !source.synthetic_fixture
            && Self::launch_target(&source.path).is_some();
        Ok(SourceValidation {
            valid,
            detected_variant: valid.then(|| source.variant_id.clone()),
            summary: if valid {
                "A Wii U RPX launch target was found in the user-selected installation."
            } else {
                "Select the legal Wii U game directory containing code/*.rpx."
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
                "The Wii U game installation could not be validated.",
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
            && Self::launch_target(&installation.root).is_some();
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Wii U installation is ready."
            } else {
                "Wii U installation or RPX launch target is missing."
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
                "Register a valid Wii U installation before launch.",
            ));
        }
        if config
            .values
            .get("graphics_backend")
            .is_some_and(|value| !["vulkan", "opengl"].contains(&value.as_str()))
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Graphics backend must be vulkan or opengl.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "The Wii U adapter only launches explicit, authorized user installations.",
            ));
        }
        self.configure(&request.installation, &request.config)?;
        let executable = self.executable()?;
        let target = Self::launch_target(&request.installation.root).ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::GameSourceMissing,
                "The Wii U RPX launch target disappeared.",
            )
        })?;
        let mut command = Command::new(executable);
        command
            .arg("-g")
            .arg(&target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if request
            .config
            .values
            .get("fullscreen")
            .is_some_and(|value| value == "true")
        {
            command.arg("-f");
        }
        let child = command.spawn().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeLaunchFailed,
                "The Cemu-compatible runtime could not be started.",
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
        let path = self
            .executable
            .as_deref()
            .and_then(Path::parent)
            .map(|root| root.join("mlc01/usr/save"));
        Ok(path.filter(|path| path.exists()).map(|path| SaveLocation {
            path,
            confidence: PlayablePrecision::Approximate,
            summary: "Cemu-compatible mlc01 save root detected; title-specific subdirectories remain runtime-managed.".to_owned(),
        }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut output = vec![RuntimeDiagnostic {
            id: "wiiu-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available)
                .then(|| "Choose your external runtime executable in game setup.".to_owned()),
        }];
        if let Some(installation) = installation {
            let result = self.verify(installation)?;
            output.push(RuntimeDiagnostic {
                id: "wiiu-game-source".to_owned(),
                severity: if result.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: result.summary,
                remediation: (!result.ready).then(|| {
                    "Choose the dumped Wii U game directory that contains code/*.rpx.".to_owned()
                }),
            });
        }
        output.push(RuntimeDiagnostic {
            id: "wiiu-playable-method".to_owned(),
            severity: DiagnosticSeverity::Warning,
            summary: "Playable detection is approximate for this external runtime.".to_owned(),
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

    const CATALOG: &str = include_str!("../../../../packages/game-catalog/catalog.v1.json");

    #[test]
    fn validates_a_user_supplied_botw_rpx_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("code")).unwrap();
        fs::write(temp.path().join("code/U-King.rpx"), b"fixture").unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let game = catalog.game(&GameId::new("zelda-botw").unwrap()).unwrap();
        let source = GameSource {
            variant_id: GameVariantId::new("wiiu").unwrap(),
            path: temp.path().to_owned(),
            synthetic_fixture: false,
        };
        assert!(
            WiiURuntimeAdapter::new(None)
                .validate_source(game, &source)
                .unwrap()
                .valid
        );
    }

    #[test]
    fn rejects_a_directory_without_a_launch_target() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let game = catalog.game(&GameId::new("zelda-botw").unwrap()).unwrap();
        let source = GameSource {
            variant_id: GameVariantId::new("wiiu").unwrap(),
            path: temp.path().to_owned(),
            synthetic_fixture: false,
        };
        assert!(
            !WiiURuntimeAdapter::new(None)
                .validate_source(game, &source)
                .unwrap()
                .valid
        );
    }
}
