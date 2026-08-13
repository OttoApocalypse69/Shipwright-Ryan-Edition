//! Catalog-backed local library state. This crate owns no runtime-specific
//! behavior and never copies or deletes user game content.

use serde::{Deserialize, Serialize};
use sre_core::{GameCatalog, GameId, GameVariantId, RuntimeId};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstallationStatus {
    NeedsSetup,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInstallation {
    pub installation_id: String,
    pub game_id: GameId,
    pub variant_id: GameVariantId,
    pub runtime_id: RuntimeId,
    pub game_source: PathBuf,
    pub runtime_executable: Option<PathBuf>,
    pub status: InstallationStatus,
    pub configured_at_unix_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryState {
    pub installations: BTreeMap<String, LibraryInstallation>,
}

#[derive(Debug)]
pub enum LibraryError {
    Io(std::io::Error),
    InvalidState(serde_json::Error),
    UnknownGame(GameId),
    UnknownVariant(GameVariantId),
    InvalidRuntime(RuntimeId),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "library storage failed: {error}"),
            Self::InvalidState(error) => write!(formatter, "library state is invalid: {error}"),
            Self::UnknownGame(id) => write!(formatter, "game {id} is not in the catalog"),
            Self::UnknownVariant(id) => {
                write!(formatter, "variant {id} is not available for this game")
            }
            Self::InvalidRuntime(id) => write!(
                formatter,
                "runtime {id} is not a candidate for this variant"
            ),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<std::io::Error> for LibraryError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub struct LibraryStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistory {
    pub sessions: Vec<sre_runtime::RuntimeSession>,
}

pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<SessionHistory, LibraryError> {
        if !self.path.exists() {
            return Ok(SessionHistory::default());
        }
        serde_json::from_slice(&fs::read(&self.path)?).map_err(LibraryError::InvalidState)
    }

    pub fn upsert(&self, session: sre_runtime::RuntimeSession) -> Result<(), LibraryError> {
        let mut history = self.load()?;
        if let Some(existing) = history
            .sessions
            .iter_mut()
            .find(|value| value.session_id == session.session_id)
        {
            *existing = session;
        } else {
            history.sessions.push(session);
        }
        let parent = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary
            .write_all(&serde_json::to_vec_pretty(&history).map_err(LibraryError::InvalidState)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| LibraryError::Io(error.error))?;
        Ok(())
    }
}

impl LibraryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<LibraryState, LibraryError> {
        if !self.path.exists() {
            return Ok(LibraryState::default());
        }
        serde_json::from_slice(&fs::read(&self.path)?).map_err(LibraryError::InvalidState)
    }

    pub fn register(
        &self,
        catalog: &GameCatalog,
        game_id: GameId,
        variant_id: GameVariantId,
        runtime_id: RuntimeId,
        game_source: PathBuf,
        runtime_executable: Option<PathBuf>,
    ) -> Result<LibraryInstallation, LibraryError> {
        let game = catalog
            .game(&game_id)
            .ok_or_else(|| LibraryError::UnknownGame(game_id.clone()))?;
        let variant = game
            .variants
            .iter()
            .find(|value| value.id == variant_id)
            .ok_or_else(|| LibraryError::UnknownVariant(variant_id.clone()))?;
        if !variant
            .runtime_candidates
            .iter()
            .any(|value| value.runtime_id == runtime_id)
        {
            return Err(LibraryError::InvalidRuntime(runtime_id));
        }
        let installation = LibraryInstallation {
            installation_id: Uuid::new_v4().to_string(),
            game_id,
            variant_id,
            runtime_id,
            game_source,
            runtime_executable,
            status: InstallationStatus::NeedsSetup,
            configured_at_unix_ms: now_unix_ms(),
            last_error: None,
        };
        let mut state = self.load()?;
        state
            .installations
            .insert(installation.installation_id.clone(), installation.clone());
        self.save(&state)?;
        Ok(installation)
    }

    pub fn save(&self, state: &LibraryState) -> Result<(), LibraryError> {
        let parent = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary
            .write_all(&serde_json::to_vec_pretty(state).map_err(LibraryError::InvalidState)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| LibraryError::Io(error.error))?;
        Ok(())
    }

    pub fn mark_status(
        &self,
        installation_id: &str,
        status: InstallationStatus,
        last_error: Option<String>,
    ) -> Result<LibraryInstallation, LibraryError> {
        let mut state = self.load()?;
        let installation = state
            .installations
            .get_mut(installation_id)
            .ok_or_else(|| {
                LibraryError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "installation does not exist",
                ))
            })?;
        installation.status = status;
        installation.last_error = last_error;
        let updated = installation.clone();
        self.save(&state)?;
        Ok(updated)
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
    const CATALOG: &str = include_str!("../../../packages/game-catalog/catalog.v1.json");

    #[test]
    fn registers_any_catalog_game_without_game_specific_logic() {
        let temp = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(temp.path().join("library.json"));
        let catalog = GameCatalog::from_json(CATALOG).unwrap();
        let result = store
            .register(
                &catalog,
                GameId::new("animal-crossing-new-horizons").unwrap(),
                GameVariantId::new("switch").unwrap(),
                RuntimeId::new("switch-runtime").unwrap(),
                temp.path().join("game.nsp"),
                Some(temp.path().join("runtime.exe")),
            )
            .unwrap();
        assert_eq!(result.game_id.as_str(), "animal-crossing-new-horizons");
        assert_eq!(store.load().unwrap().installations.len(), 1);
    }
}
