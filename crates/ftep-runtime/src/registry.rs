use crate::{DetectionStatus, RuntimeError, RuntimeProvider};
use ftep_core::{GameVariant, RuntimeId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateProvider(RuntimeId),
    UnknownProvider(RuntimeId),
    DetectionFailed {
        runtime_id: RuntimeId,
        error: RuntimeError,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProvider(id) => {
                write!(formatter, "runtime provider {id} is already registered")
            }
            Self::UnknownProvider(id) => {
                write!(formatter, "runtime provider {id} is not registered")
            }
            Self::DetectionFailed { runtime_id, error } => {
                write!(
                    formatter,
                    "runtime provider {runtime_id} detection failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone)]
pub struct ResolvedProvider {
    pub provider: Arc<dyn RuntimeProvider>,
    pub user_preferred: bool,
    pub catalog_priority: u16,
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<RuntimeId, Arc<dyn RuntimeProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Arc<dyn RuntimeProvider>) -> Result<(), RegistryError> {
        let id = provider.id().clone();
        if self.providers.contains_key(&id) {
            return Err(RegistryError::DuplicateProvider(id));
        }
        self.providers.insert(id, provider);
        Ok(())
    }

    pub fn get(&self, id: &RuntimeId) -> Option<Arc<dyn RuntimeProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn registered_ids(&self) -> impl Iterator<Item = &RuntimeId> {
        self.providers.keys()
    }

    pub fn resolve_candidates(
        &self,
        variant: &GameVariant,
        user_preference: Option<&RuntimeId>,
    ) -> Vec<ResolvedProvider> {
        let candidate_ids = variant
            .runtime_candidates
            .iter()
            .map(|candidate| &candidate.runtime_id)
            .collect::<BTreeSet<_>>();
        let effective_preference = user_preference
            .filter(|id| candidate_ids.contains(id))
            .or(variant.preferred_runtime.as_ref());

        let mut resolved = variant
            .runtime_candidates
            .iter()
            .filter_map(|candidate| {
                self.providers
                    .get(&candidate.runtime_id)
                    .cloned()
                    .map(|provider| ResolvedProvider {
                        provider,
                        user_preferred: user_preference == Some(&candidate.runtime_id),
                        catalog_priority: candidate.priority,
                    })
            })
            .collect::<Vec<_>>();

        resolved.sort_by_key(|entry| {
            (
                effective_preference != Some(entry.provider.id()),
                entry.catalog_priority,
                entry.provider.id().clone(),
            )
        });
        resolved
    }

    pub fn first_available(
        &self,
        variant: &GameVariant,
        user_preference: Option<&RuntimeId>,
    ) -> Result<Option<ResolvedProvider>, RegistryError> {
        for candidate in self.resolve_candidates(variant, user_preference) {
            let detection =
                candidate
                    .provider
                    .detect()
                    .map_err(|error| RegistryError::DetectionFailed {
                        runtime_id: candidate.provider.id().clone(),
                        error,
                    })?;
            if detection.status == DetectionStatus::Available {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRuntime;
    use ftep_core::{CompatibilityState, GameVariantId, OriginalPlatform, RuntimeCandidate};

    fn variant() -> GameVariant {
        GameVariant {
            id: GameVariantId::new("fixture").unwrap(),
            original_platform: OriginalPlatform::Other,
            runtime_candidates: vec![
                RuntimeCandidate {
                    runtime_id: RuntimeId::new("slow-choice").unwrap(),
                    priority: 100,
                    compatibility: CompatibilityState::Experimental,
                },
                RuntimeCandidate {
                    runtime_id: RuntimeId::new("preferred-choice").unwrap(),
                    priority: 10,
                    compatibility: CompatibilityState::Supported,
                },
            ],
            preferred_runtime: Some(RuntimeId::new("preferred-choice").unwrap()),
            source_requirements: vec![],
        }
    }

    #[test]
    fn rejects_duplicate_provider_ids() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(Arc::new(SyntheticRuntime::available("same-runtime")))
            .unwrap();
        assert!(matches!(
            registry.register(Arc::new(SyntheticRuntime::available("same-runtime"))),
            Err(RegistryError::DuplicateProvider(_))
        ));
    }

    #[test]
    fn resolves_catalog_preference_without_game_specific_branches() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(Arc::new(SyntheticRuntime::available("slow-choice")))
            .unwrap();
        registry
            .register(Arc::new(SyntheticRuntime::available("preferred-choice")))
            .unwrap();

        let resolved = registry.resolve_candidates(&variant(), None);
        assert_eq!(resolved[0].provider.id().as_str(), "preferred-choice");
        assert_eq!(resolved[1].provider.id().as_str(), "slow-choice");
    }

    #[test]
    fn explicit_user_preference_wins_but_is_never_invented() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(Arc::new(SyntheticRuntime::available("slow-choice")))
            .unwrap();
        registry
            .register(Arc::new(SyntheticRuntime::available("preferred-choice")))
            .unwrap();

        let user = RuntimeId::new("slow-choice").unwrap();
        let resolved = registry.resolve_candidates(&variant(), Some(&user));
        assert_eq!(resolved[0].provider.id(), &user);
        assert!(resolved[0].user_preferred);

        let unrelated = RuntimeId::new("not-a-candidate").unwrap();
        let resolved = registry.resolve_candidates(&variant(), Some(&unrelated));
        assert_eq!(resolved[0].provider.id().as_str(), "preferred-choice");
    }
}
