use ftep_core::{GameDefinition, GameId, GameVariantId, RuntimeId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeKind {
    NativePort,
    ExternalEmulator,
    ManagedRuntime,
    ExternalRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DistributionMode {
    Bundled,
    ManagedDownload,
    External,
    ManualOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub native: bool,
    pub external_process: bool,
    pub supports_overlay: bool,
    pub supports_semantic_events: bool,
    pub supports_save_detection: bool,
    pub supports_controller_config: bool,
    pub supports_mods: bool,
}

impl RuntimeCapabilities {
    pub const fn none() -> Self {
        Self {
            native: false,
            external_process: false,
            supports_overlay: false,
            supports_semantic_events: false,
            supports_save_detection: false,
            supports_controller_config: false,
            supports_mods: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DetectionStatus {
    Available,
    Missing,
    Incompatible,
    KnownBroken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInstallation {
    pub runtime_id: RuntimeId,
    pub root: PathBuf,
    pub executable: Option<PathBuf>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub runtime_id: RuntimeId,
    pub status: DetectionStatus,
    pub installation: Option<RuntimeInstallation>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSource {
    pub variant_id: GameVariantId,
    pub path: PathBuf,
    pub synthetic_fixture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceValidation {
    pub valid: bool,
    pub detected_variant: Option<GameVariantId>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationContext {
    pub staging_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedGame {
    pub game_id: GameId,
    pub variant_id: GameVariantId,
    pub runtime_id: RuntimeId,
    pub prepared_root: PathBuf,
    pub synthetic_fixture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstallation {
    pub installation_id: String,
    pub game_id: GameId,
    pub variant_id: GameVariantId,
    pub runtime_id: RuntimeId,
    pub root: PathBuf,
    pub synthetic_fixture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub ready: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LaunchMode {
    Authorized,
    SyntheticFixture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub session_id: String,
    pub installation: GameInstallation,
    pub config: RuntimeConfig,
    pub mode: LaunchMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayablePrecision {
    Exact,
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeEvent {
    RuntimeReady {
        at_unix_ms: u64,
    },
    GameBooted {
        at_unix_ms: u64,
    },
    Playable {
        at_unix_ms: u64,
        precision: PlayablePrecision,
    },
    SaveLoaded {
        at_unix_ms: u64,
    },
    SessionEnding {
        at_unix_ms: u64,
    },
    GameExited {
        at_unix_ms: u64,
        exit_code: Option<i32>,
    },
    AchievementEvent {
        at_unix_ms: u64,
        event_id: String,
    },
    Error {
        at_unix_ms: u64,
        code: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeSessionState {
    Requested,
    Authorizing,
    Preparing,
    StartingRuntime,
    RuntimeStarted,
    Playable,
    Ending,
    Ended,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSession {
    pub session_id: String,
    pub game_id: GameId,
    pub runtime_id: RuntimeId,
    pub state: RuntimeSessionState,
    pub process_id: Option<u32>,
    pub synthetic_fixture: bool,
    pub initial_events: Vec<RuntimeEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeErrorCode {
    GameNotConfigured,
    RuntimeNotInstalled,
    RuntimeVersionUnsupported,
    RuntimeKnownBroken,
    RuntimeLaunchFailed,
    RuntimeExitedImmediately,
    RuntimeTimeout,
    RuntimeConfigurationInvalid,
    GameSourceMissing,
    GameSourceInvalid,
    SyntheticLaunchForbidden,
    OperationUnsupported,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub message: String,
    pub technical_details: Option<String>,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            technical_details: None,
        }
    }

    pub fn with_technical_details(mut self, details: impl Into<String>) -> Self {
        self.technical_details = Some(details.into());
        self
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub trait RuntimeProvider: Send + Sync {
    fn id(&self) -> &RuntimeId;
    fn display_name(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn distribution_mode(&self) -> DistributionMode;
    fn capabilities(&self) -> RuntimeCapabilities;
    fn detect(&self) -> Result<DetectionResult, RuntimeError>;
    fn validate_source(
        &self,
        game: &GameDefinition,
        source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError>;
    fn prepare(
        &self,
        game: &GameDefinition,
        source: &GameSource,
        context: &PreparationContext,
    ) -> Result<PreparedGame, RuntimeError>;
    fn verify(&self, installation: &GameInstallation) -> Result<VerificationResult, RuntimeError>;
    fn configure(
        &self,
        installation: &GameInstallation,
        config: &RuntimeConfig,
    ) -> Result<(), RuntimeError>;
    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError>;
}
