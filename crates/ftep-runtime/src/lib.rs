//! Stable runtime-provider interfaces owned by FTEP.
//!
//! Concrete native ports and external emulator integrations implement these
//! contracts. This crate never depends on a concrete provider.

mod provider;
mod registry;
pub mod synthetic;

pub use provider::{
    DetectionResult, DetectionStatus, DistributionMode, GameInstallation, GameSource, LaunchMode,
    LaunchRequest, PlayablePrecision, PreparationContext, PreparedGame, RuntimeCapabilities,
    RuntimeConfig, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeInstallation, RuntimeKind,
    RuntimeProvider, RuntimeSession, RuntimeSessionState, SourceValidation, VerificationResult,
};
pub use registry::{ProviderRegistry, RegistryError, ResolvedProvider};
