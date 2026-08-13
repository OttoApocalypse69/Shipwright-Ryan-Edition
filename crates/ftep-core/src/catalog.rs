use crate::{FranchiseId, GameId, GameVariantId, RuntimeId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompatibilityState {
    Unsupported,
    Investigating,
    Experimental,
    Supported,
    Degraded,
    Broken,
    Deprecated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OriginalPlatform {
    Nintendo64,
    SuperNintendo,
    GameCube,
    Wii,
    WiiU,
    Switch,
    NintendoDs,
    Nintendo3ds,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceKind {
    GameImage,
    RuntimeMaterial,
    Directory,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRequirement {
    pub id: String,
    pub kind: SourceKind,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCandidate {
    pub runtime_id: RuntimeId,
    pub priority: u16,
    pub compatibility: CompatibilityState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameVariant {
    pub id: GameVariantId,
    pub original_platform: OriginalPlatform,
    pub runtime_candidates: Vec<RuntimeCandidate>,
    pub preferred_runtime: Option<RuntimeId>,
    pub source_requirements: Vec<SourceRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameDefinition {
    pub id: GameId,
    pub franchise: FranchiseId,
    pub title: String,
    pub variants: Vec<GameVariant>,
    pub preferred_variant: GameVariantId,
    pub achievement_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameCatalog {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub schema_version: u32,
    pub games: Vec<GameDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError(String);

impl CatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CatalogError {}

impl GameCatalog {
    pub fn from_json(input: &str) -> Result<Self, CatalogError> {
        let catalog: Self = serde_json::from_str(input)
            .map_err(|error| CatalogError::new(format!("catalog JSON is invalid: {error}")))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), CatalogError> {
        if self.schema_version != 1 {
            return Err(CatalogError::new(format!(
                "unsupported game catalog schema version {}",
                self.schema_version
            )));
        }
        if self.games.is_empty() {
            return Err(CatalogError::new(
                "game catalog must contain at least one game",
            ));
        }

        let mut game_ids = BTreeSet::new();
        for game in &self.games {
            if !game_ids.insert(game.id.clone()) {
                return Err(CatalogError::new(format!("duplicate game id {}", game.id)));
            }
            validate_game(game)?;
        }
        Ok(())
    }

    pub fn game(&self, id: &GameId) -> Option<&GameDefinition> {
        self.games.iter().find(|game| &game.id == id)
    }
}

fn validate_game(game: &GameDefinition) -> Result<(), CatalogError> {
    if game.title.trim().is_empty() {
        return Err(CatalogError::new(format!(
            "game {} has an empty title",
            game.id
        )));
    }
    if !valid_namespace(&game.achievement_namespace) {
        return Err(CatalogError::new(format!(
            "game {} has invalid achievement namespace {:?}",
            game.id, game.achievement_namespace
        )));
    }
    if game.variants.is_empty() {
        return Err(CatalogError::new(format!(
            "game {} must define at least one variant",
            game.id
        )));
    }

    let mut variant_ids = BTreeSet::new();
    for variant in &game.variants {
        if !variant_ids.insert(variant.id.clone()) {
            return Err(CatalogError::new(format!(
                "game {} has duplicate variant {}",
                game.id, variant.id
            )));
        }
        validate_variant(game, variant)?;
    }
    if !variant_ids.contains(&game.preferred_variant) {
        return Err(CatalogError::new(format!(
            "game {} prefers unknown variant {}",
            game.id, game.preferred_variant
        )));
    }
    Ok(())
}

fn validate_variant(game: &GameDefinition, variant: &GameVariant) -> Result<(), CatalogError> {
    if variant.runtime_candidates.is_empty() {
        return Err(CatalogError::new(format!(
            "game {} variant {} has no runtime candidates",
            game.id, variant.id
        )));
    }

    let mut runtime_ids = BTreeSet::new();
    let mut priorities = BTreeSet::new();
    for candidate in &variant.runtime_candidates {
        if !runtime_ids.insert(candidate.runtime_id.clone()) {
            return Err(CatalogError::new(format!(
                "game {} variant {} repeats runtime {}",
                game.id, variant.id, candidate.runtime_id
            )));
        }
        if !priorities.insert(candidate.priority) {
            return Err(CatalogError::new(format!(
                "game {} variant {} repeats runtime priority {}",
                game.id, variant.id, candidate.priority
            )));
        }
    }
    if let Some(preferred) = &variant.preferred_runtime {
        if !runtime_ids.contains(preferred) {
            return Err(CatalogError::new(format!(
                "game {} variant {} prefers unknown runtime {}",
                game.id, variant.id, preferred
            )));
        }
    }

    let mut requirement_ids = BTreeSet::new();
    for requirement in &variant.source_requirements {
        if requirement.id.trim().is_empty() || requirement.description.trim().is_empty() {
            return Err(CatalogError::new(format!(
                "game {} variant {} has an empty source requirement",
                game.id, variant.id
            )));
        }
        if !requirement_ids.insert(requirement.id.as_str()) {
            return Err(CatalogError::new(format!(
                "game {} variant {} repeats source requirement {}",
                game.id, variant.id, requirement.id
            )));
        }
    }
    Ok(())
}

fn valid_namespace(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && !segment.contains("--")
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
    {
      "schemaVersion": 1,
      "games": [{
        "id": "fixture-game",
        "franchise": "fixture",
        "title": "Fixture Game",
        "variants": [{
          "id": "fixture",
          "originalPlatform": "OTHER",
          "runtimeCandidates": [{
            "runtimeId": "fake-runtime",
            "priority": 10,
            "compatibility": "EXPERIMENTAL"
          }],
          "preferredRuntime": "fake-runtime",
          "sourceRequirements": []
        }],
        "preferredVariant": "fixture",
        "achievementNamespace": "ftep.game.fixture-game"
      }]
    }"#;

    #[test]
    fn parses_and_queries_a_valid_catalog() {
        let catalog = GameCatalog::from_json(MINIMAL).unwrap();
        let id = GameId::new("fixture-game").unwrap();
        assert_eq!(catalog.game(&id).unwrap().title, "Fixture Game");
    }

    #[test]
    fn rejects_unknown_preferred_runtime() {
        let input = MINIMAL.replace(
            "\"preferredRuntime\": \"fake-runtime\"",
            "\"preferredRuntime\": \"missing-runtime\"",
        );
        assert!(
            GameCatalog::from_json(&input)
                .unwrap_err()
                .to_string()
                .contains("prefers unknown runtime")
        );
    }
}
