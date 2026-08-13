//! Stable runtime-provider interfaces owned by SRE.
//!
//! Concrete native ports and external emulator integrations implement these
//! contracts. This crate never depends on a concrete provider.

mod provider;
mod registry;
pub mod synthetic;

pub use provider::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeInstallation,
    RuntimeKind, RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation,
    SourceValidation, VerificationResult, VersionStatus, VersionValidation,
};
pub use registry::{ProviderRegistry, RegistryError, ResolvedProvider};
