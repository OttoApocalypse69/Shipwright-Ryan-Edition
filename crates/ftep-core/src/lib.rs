//! Provider-neutral FTEP domain values.
//!
//! This crate deliberately has no knowledge of Tauri, Shipwright, emulators,
//! operating-system process APIs, or repository filesystem layout.

mod catalog;
mod identifier;

pub use catalog::{
    CatalogError, CompatibilityState, GameCatalog, GameDefinition, GameVariant, OriginalPlatform,
    RuntimeCandidate, SourceKind, SourceRequirement,
};
pub use identifier::{FranchiseId, GameId, GameVariantId, IdentifierError, RuntimeId};
