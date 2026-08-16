use crate::storage;
use serde::Deserialize;
use serde_json::Value;
use sre_core::{GameCatalog, GameId, GameVariantId, RuntimeId};
use sre_dolphin_adapter::DolphinRuntimeAdapter;
use sre_library::{InstallationStatus, LibraryInstallation, LibraryState, LibraryStore};
use sre_runtime::{GameSource, RuntimeProvider};
use sre_switch_adapter::{ExternalSwitchImplementation, SwitchRuntimeProvider};
use sre_two_ship_adapter::TwoShipAdapter;
use sre_wiiu_adapter::WiiURuntimeAdapter;
use std::path::PathBuf;

const CATALOG_JSON: &str = include_str!("../../../../packages/game-catalog/catalog.v1.json");

fn catalog() -> Result<GameCatalog, String> {
    GameCatalog::from_json(CATALOG_JSON)
        .map_err(|error| format!("FICSIT-0001: Catalog validation failed: {error}"))
}
fn store(app: &tauri::AppHandle) -> Result<LibraryStore, String> {
    Ok(LibraryStore::new(
        storage::app_data_dir(app)?.join("library.json"),
    ))
}

#[tauri::command]
pub(crate) fn game_catalog() -> Result<Value, String> {
    serde_json::from_str(CATALOG_JSON).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn load_library(app: tauri::AppHandle) -> Result<LibraryState, String> {
    store(&app)?.load().map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegisterGameRequest {
    game_id: String,
    variant_id: String,
    runtime_id: String,
    game_source: String,
    runtime_executable: Option<String>,
    switch_implementation_id: Option<String>,
}

#[tauri::command]
pub(crate) fn register_game(
    app: tauri::AppHandle,
    request: RegisterGameRequest,
) -> Result<LibraryInstallation, String> {
    let catalog = catalog()?;
    let game_id = GameId::new(request.game_id).map_err(|error| error.to_string())?;
    let variant_id = GameVariantId::new(request.variant_id).map_err(|error| error.to_string())?;
    let runtime_id = RuntimeId::new(request.runtime_id).map_err(|error| error.to_string())?;
    let game = catalog
        .game(&game_id)
        .ok_or_else(|| "FICSIT-0007: The selected game is not in the SRE catalog.".to_owned())?;
    let source_path = PathBuf::from(request.game_source);
    let mut runtime_path = request.runtime_executable.map(PathBuf::from);
    if runtime_id.as_str() == "cemu-compatible" && runtime_path.is_none() {
        runtime_path = Some(crate::importer::managed_cemu_executable(&app)?);
    }
    if runtime_id.as_str() == "switch-runtime" && runtime_path.is_none() {
        runtime_path = Some(crate::importer::managed_ryujinx_executable(&app)?);
    }
    let source = GameSource {
        variant_id: variant_id.clone(),
        path: source_path.clone(),
        synthetic_fixture: false,
    };
    let validation = match runtime_id.as_str() {
        "cemu-compatible" => {
            WiiURuntimeAdapter::new(runtime_path.clone()).validate_source(game, &source)
        }
        "switch-runtime" => {
            let implementation = runtime_path.clone().map(|path| {
                ExternalSwitchImplementation::manually_configured(
                    request
                        .switch_implementation_id
                        .unwrap_or_else(|| "user-selected".to_owned()),
                    "User-selected Switch runtime",
                    path,
                )
            });
            SwitchRuntimeProvider::new(implementation).validate_source(game, &source)
        }
        "dolphin-compatible" => {
            DolphinRuntimeAdapter::new(runtime_path.clone()).validate_source(game, &source)
        }
        "shipwright" => {
            return Err(
                "FICSIT-0006: Use the guided native OoT import workflow for Shipwright game data."
                    .to_owned(),
            );
        }
        "two-ship" => {
            // Registration validates the original user file only. Runtime staging is
            // deliberately deferred to the background launch worker.
            let provider = TwoShipAdapter::new(crate::importer::two_ship_executable(&app)?);
            provider.validate_source(game, &source)
        }
        _ => {
            return Err(
                "FICSIT-0008: This catalog runtime has no release-candidate adapter.".to_owned(),
            );
        }
    }
    .map_err(|error| error.to_string())?;
    if !validation.valid {
        return Err(format!("FICSIT-0007: {}", validation.summary));
    }
    if matches!(
        runtime_id.as_str(),
        "cemu-compatible" | "switch-runtime" | "dolphin-compatible"
    ) && !runtime_path.as_ref().is_some_and(|path| path.is_file())
    {
        return Err("FICSIT-0008: The managed runtime is missing. Reinstall SRE.".to_owned());
    }
    let store = store(&app)?;
    let installation = store
        .register(
            &catalog,
            game_id,
            variant_id,
            runtime_id,
            source_path,
            runtime_path,
        )
        .map_err(|error| error.to_string())?;
    store
        .mark_status(
            &installation.installation_id,
            InstallationStatus::Ready,
            None,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn register_imported_shipwright_game(
    app: tauri::AppHandle,
) -> Result<LibraryInstallation, String> {
    let catalog = catalog()?;
    let game_id = GameId::new("zelda-oot").expect("the built-in OoT id must be valid");
    let variant_id = GameVariantId::new("n64").expect("the built-in N64 id must be valid");
    let runtime_id = RuntimeId::new("shipwright").expect("the built-in runtime id must be valid");
    let imported_assets = crate::importer::current_asset_directory(&app)?.ok_or_else(|| {
        "FICSIT-0006: Import compatible OoT game data before adding it to the library.".to_owned()
    })?;
    let store = store(&app)?;
    let installation = store
        .register(
            &catalog,
            game_id,
            variant_id,
            runtime_id,
            imported_assets,
            None,
        )
        .map_err(|error| error.to_string())?;
    store
        .mark_status(
            &installation.installation_id,
            InstallationStatus::Ready,
            None,
        )
        .map_err(|error| error.to_string())
}
