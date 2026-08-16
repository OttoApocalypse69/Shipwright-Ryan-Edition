//! Deterministic, non-playable runtime providers for tests and UI development.

use crate::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeInstallation,
    RuntimeKind, RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation,
    SourceValidation, VerificationResult, VersionStatus, VersionValidation,
};
use sre_core::{GameDefinition, RuntimeId};
use std::path::PathBuf;

pub struct SyntheticRuntime {
    id: RuntimeId,
    display_name: String,
    status: DetectionStatus,
    playable_precision: PlayablePrecision,
    reported_version: String,
    launch_fails: bool,
    emits_playable: bool,
}

impl SyntheticRuntime {
    pub fn available(id: &str) -> Self {
        Self {
            id: RuntimeId::new(id).expect("synthetic runtime id must be valid"),
            display_name: format!("Synthetic {id}"),
            status: DetectionStatus::Available,
            playable_precision: PlayablePrecision::Exact,
            reported_version: "fixture-1".to_owned(),
            launch_fails: false,
            emits_playable: true,
        }
    }

    pub fn missing(id: &str) -> Self {
        Self {
            status: DetectionStatus::Missing,
            ..Self::available(id)
        }
    }

    pub fn with_playable_precision(mut self, precision: PlayablePrecision) -> Self {
        self.playable_precision = precision;
        self
    }

    pub fn with_reported_version(mut self, version: impl Into<String>) -> Self {
        self.reported_version = version.into();
        self
    }

    pub fn with_launch_failure(mut self) -> Self {
        self.launch_fails = true;
        self
    }

    pub fn without_playable_signal(mut self) -> Self {
        self.emits_playable = false;
        self
    }
}

pub fn fake_native_runtime() -> SyntheticRuntime {
    SyntheticRuntime::available("fake-native-runtime")
}
pub fn fake_wiiu_runtime() -> SyntheticRuntime {
    SyntheticRuntime::available("fake-wiiu-runtime")
        .with_playable_precision(PlayablePrecision::Approximate)
}
pub fn fake_switch_runtime() -> SyntheticRuntime {
    SyntheticRuntime::available("fake-switch-runtime")
        .with_playable_precision(PlayablePrecision::Approximate)
}
pub fn broken_runtime() -> SyntheticRuntime {
    SyntheticRuntime {
        status: DetectionStatus::KnownBroken,
        ..SyntheticRuntime::available("broken-runtime")
    }
}
pub fn slow_runtime() -> SyntheticRuntime {
    SyntheticRuntime::available("slow-runtime").without_playable_signal()
}
pub fn runtime_without_playable_signal() -> SyntheticRuntime {
    SyntheticRuntime::available("no-playable-runtime").without_playable_signal()
}

impl RuntimeProvider for SyntheticRuntime {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::ManagedRuntime
    }

    fn distribution_mode(&self) -> DistributionMode {
        DistributionMode::ManualOnly
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            native: false,
            external_process: false,
            supports_overlay: true,
            supports_semantic_events: true,
            supports_save_detection: false,
            supports_controller_config: false,
            supports_mods: false,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let installation =
            (self.status == DetectionStatus::Available).then(|| RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: PathBuf::from(".ftep-synthetic-runtime"),
                executable: None,
                version: Some(self.reported_version.clone()),
            });
        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: self.status,
            installation,
            summary: format!("{} is {:?}", self.display_name, self.status),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        let supported = installation.version.as_deref() == Some("fixture-1");
        Ok(VersionValidation {
            status: if supported {
                VersionStatus::Supported
            } else {
                VersionStatus::Unsupported
            },
            detected_version: installation.version.clone(),
            summary: if supported {
                "Synthetic fixture version is supported."
            } else {
                "Synthetic fixture version is unsupported."
            }
            .to_owned(),
        })
    }

    fn validate_source(
        &self,
        _game: &GameDefinition,
        source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError> {
        Ok(SourceValidation {
            valid: source.synthetic_fixture,
            detected_variant: source.synthetic_fixture.then(|| source.variant_id.clone()),
            summary: if source.synthetic_fixture {
                "Synthetic fixture accepted; no proprietary data was read."
            } else {
                "Synthetic providers refuse real game data."
            }
            .to_owned(),
        })
    }

    fn prepare(
        &self,
        game: &GameDefinition,
        source: &GameSource,
        context: &PreparationContext,
    ) -> Result<PreparedGame, RuntimeError> {
        if !source.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "Synthetic providers only accept explicit fixture sources.",
            ));
        }
        Ok(PreparedGame {
            game_id: game.id.clone(),
            variant_id: source.variant_id.clone(),
            runtime_id: self.id.clone(),
            prepared_root: context.staging_root.clone(),
            synthetic_fixture: true,
        })
    }

    fn verify(&self, installation: &GameInstallation) -> Result<VerificationResult, RuntimeError> {
        let ready = installation.synthetic_fixture && installation.runtime_id == self.id;
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Synthetic fixture is ready."
            } else {
                "Installation is not an explicit fixture for this provider."
            }
            .to_owned(),
        })
    }

    fn configure(
        &self,
        installation: &GameInstallation,
        _config: &RuntimeConfig,
    ) -> Result<(), RuntimeError> {
        if !installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Synthetic providers cannot configure real installations.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        if request.mode != LaunchMode::SyntheticFixture || !request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "Synthetic runtime sessions cannot launch real game data.",
            ));
        }
        if request.installation.runtime_id != self.id {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "The fixture installation belongs to another provider.",
            ));
        }
        if self.launch_fails {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeLaunchFailed,
                "Synthetic runtime launch failure fixture.",
            ));
        }

        let state = if self.emits_playable {
            RuntimeSessionState::Playable
        } else {
            RuntimeSessionState::RuntimeStarted
        };
        let mut events = vec![
            RuntimeEvent::RuntimeReady { at_unix_ms: 0 },
            RuntimeEvent::GameBooted { at_unix_ms: 0 },
        ];
        if self.emits_playable {
            events.push(RuntimeEvent::Playable {
                at_unix_ms: 0,
                precision: self.playable_precision,
            });
        }

        Ok(RuntimeSession {
            session_id: request.session_id,
            game_id: request.installation.game_id,
            variant_id: request.installation.variant_id,
            runtime_id: self.id.clone(),
            device_id: Some("synthetic-device".to_owned()),
            state,
            process_id: None,
            synthetic_fixture: true,
            requested_at_unix_ms: 0,
            started_at_unix_ms: Some(0),
            playable_at_unix_ms: self.emits_playable.then_some(0),
            ended_at_unix_ms: None,
            duration_ms: None,
            exit_code: None,
            launch_result: if self.emits_playable {
                "SYNTHETIC_PLAYABLE"
            } else {
                "SYNTHETIC_NO_PLAYABLE_SIGNAL"
            }
            .to_owned(),
            initial_events: events,
        })
    }

    fn observe_process(
        &self,
        _session: &RuntimeSession,
    ) -> Result<ProcessObservation, RuntimeError> {
        Ok(ProcessObservation {
            running: true,
            observed_at_unix_ms: 0,
            exit_code: None,
        })
    }

    fn determine_playable_state(
        &self,
        session: &RuntimeSession,
    ) -> Result<PlayableDetection, RuntimeError> {
        Ok(PlayableDetection {
            playable: self.emits_playable && session.state == RuntimeSessionState::Playable,
            precision: self.playable_precision,
            method: "synthetic semantic signal".to_owned(),
            observed_at_unix_ms: 0,
        })
    }

    fn find_save_location(
        &self,
        _installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        Ok(None)
    }

    fn diagnostics(
        &self,
        _installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        Ok(vec![RuntimeDiagnostic {
            id: "synthetic-runtime".to_owned(),
            severity: DiagnosticSeverity::Info,
            summary: "Synthetic runtime is isolated from proprietary content.".to_owned(),
            remediation: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sre_core::{
        CompatibilityState, CoverMetadata, FranchiseId, GameId, GameVariant, GameVariantId,
        OriginalPlatform, RuntimeCandidate,
    };
    use std::collections::BTreeMap;

    fn game() -> GameDefinition {
        let variant_id = GameVariantId::new("fixture").unwrap();
        GameDefinition {
            id: GameId::new("fixture-game").unwrap(),
            franchise: FranchiseId::new("fixture").unwrap(),
            title: "Fixture Game".to_owned(),
            description: "Synthetic runtime fixture.".to_owned(),
            cover: CoverMetadata {
                kind: "ORIGINAL_PLACEHOLDER".to_owned(),
                title_mark: "FG".to_owned(),
                accent_color: "#55c2ff".to_owned(),
                background_color: "#101827".to_owned(),
                banner_url: None,
            },
            variants: vec![GameVariant {
                id: variant_id.clone(),
                original_platform: OriginalPlatform::Other,
                runtime_candidates: vec![RuntimeCandidate {
                    runtime_id: RuntimeId::new("fake-native-runtime").unwrap(),
                    priority: 10,
                    compatibility: CompatibilityState::Experimental,
                }],
                preferred_runtime: Some(RuntimeId::new("fake-native-runtime").unwrap()),
                source_requirements: vec![],
            }],
            preferred_variant: variant_id,
            achievement_namespace: "ftep.game.fixture-game".to_owned(),
        }
    }

    fn installation() -> GameInstallation {
        GameInstallation {
            installation_id: "fixture-installation".to_owned(),
            game_id: GameId::new("fixture-game").unwrap(),
            variant_id: GameVariantId::new("fixture").unwrap(),
            runtime_id: RuntimeId::new("fake-native-runtime").unwrap(),
            root: PathBuf::from("fixture"),
            synthetic_fixture: true,
        }
    }

    #[test]
    fn emits_deterministic_playable_event_without_a_process() {
        let provider = SyntheticRuntime::available("fake-native-runtime");
        let session = provider
            .launch(LaunchRequest {
                session_id: "session-1".to_owned(),
                installation: installation(),
                config: RuntimeConfig {
                    values: BTreeMap::new(),
                },
                mode: LaunchMode::SyntheticFixture,
            })
            .unwrap();

        assert_eq!(session.state, RuntimeSessionState::Playable);
        assert_eq!(session.process_id, None);
        assert!(matches!(
            session.initial_events.last(),
            Some(RuntimeEvent::Playable {
                precision: PlayablePrecision::Exact,
                ..
            })
        ));
    }

    #[test]
    fn refuses_real_sources_and_authorized_launch_mode() {
        let provider = SyntheticRuntime::available("fake-native-runtime");
        let source = GameSource {
            variant_id: GameVariantId::new("fixture").unwrap(),
            path: PathBuf::from("real-game-data"),
            synthetic_fixture: false,
        };
        assert!(!provider.validate_source(&game(), &source).unwrap().valid);

        let error = provider
            .launch(LaunchRequest {
                session_id: "session-2".to_owned(),
                installation: installation(),
                config: RuntimeConfig::default(),
                mode: LaunchMode::Authorized,
            })
            .unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::SyntheticLaunchForbidden);
    }

    #[test]
    fn critical_runtime_fixture_matrix_is_deterministic() {
        assert_eq!(
            SyntheticRuntime::missing("missing-runtime")
                .detect()
                .unwrap()
                .status,
            DetectionStatus::Missing
        );
        assert_eq!(
            broken_runtime().detect().unwrap().status,
            DetectionStatus::KnownBroken
        );
        let wrong = SyntheticRuntime::available("fake-native-runtime")
            .with_reported_version("wrong-version");
        let detected = wrong.detect().unwrap().installation.unwrap();
        assert_eq!(
            wrong.validate_version(&detected).unwrap().status,
            VersionStatus::Unsupported
        );

        let failing = SyntheticRuntime::available("fake-native-runtime").with_launch_failure();
        let error = failing
            .launch(LaunchRequest {
                session_id: "failure".to_owned(),
                installation: installation(),
                config: RuntimeConfig::default(),
                mode: LaunchMode::SyntheticFixture,
            })
            .unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::RuntimeLaunchFailed);

        let no_signal = runtime_without_playable_signal();
        let mut no_signal_installation = installation();
        no_signal_installation.runtime_id = no_signal.id().clone();
        let session = no_signal
            .launch(LaunchRequest {
                session_id: "timeout".to_owned(),
                installation: no_signal_installation,
                config: RuntimeConfig::default(),
                mode: LaunchMode::SyntheticFixture,
            })
            .unwrap();
        assert_eq!(session.state, RuntimeSessionState::RuntimeStarted);
        assert!(
            !no_signal
                .determine_playable_state(&session)
                .unwrap()
                .playable
        );
        assert!(
            session
                .initial_events
                .iter()
                .all(|event| !matches!(event, RuntimeEvent::Playable { .. }))
        );
    }
}
