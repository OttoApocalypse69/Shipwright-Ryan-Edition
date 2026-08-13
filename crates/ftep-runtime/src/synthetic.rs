//! Deterministic, non-playable runtime providers for tests and UI development.

use crate::{
    DetectionResult, DetectionStatus, DistributionMode, GameInstallation, GameSource, LaunchMode,
    LaunchRequest, PlayablePrecision, PreparationContext, PreparedGame, RuntimeCapabilities,
    RuntimeConfig, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeInstallation, RuntimeKind,
    RuntimeProvider, RuntimeSession, RuntimeSessionState, SourceValidation, VerificationResult,
};
use ftep_core::{GameDefinition, RuntimeId};
use std::path::PathBuf;

pub struct SyntheticRuntime {
    id: RuntimeId,
    display_name: String,
    status: DetectionStatus,
    playable_precision: PlayablePrecision,
}

impl SyntheticRuntime {
    pub fn available(id: &str) -> Self {
        Self {
            id: RuntimeId::new(id).expect("synthetic runtime id must be valid"),
            display_name: format!("Synthetic {id}"),
            status: DetectionStatus::Available,
            playable_precision: PlayablePrecision::Exact,
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
                version: Some("fixture-1".to_owned()),
            });
        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: self.status,
            installation,
            summary: format!("{} is {:?}", self.display_name, self.status),
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

        Ok(RuntimeSession {
            session_id: request.session_id,
            game_id: request.installation.game_id,
            runtime_id: self.id.clone(),
            state: RuntimeSessionState::Playable,
            process_id: None,
            synthetic_fixture: true,
            initial_events: vec![
                RuntimeEvent::RuntimeReady { at_unix_ms: 0 },
                RuntimeEvent::GameBooted { at_unix_ms: 0 },
                RuntimeEvent::Playable {
                    at_unix_ms: 0,
                    precision: self.playable_precision,
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftep_core::{
        CompatibilityState, FranchiseId, GameId, GameVariant, GameVariantId, OriginalPlatform,
        RuntimeCandidate,
    };
    use std::collections::BTreeMap;

    fn game() -> GameDefinition {
        let variant_id = GameVariantId::new("fixture").unwrap();
        GameDefinition {
            id: GameId::new("fixture-game").unwrap(),
            franchise: FranchiseId::new("fixture").unwrap(),
            title: "Fixture Game".to_owned(),
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
}
