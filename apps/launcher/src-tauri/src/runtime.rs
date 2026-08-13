use crate::{importer, platform};
use serde::{Deserialize, Serialize};
use sre_core::{GameId, GameVariantId, RuntimeId};
use sre_runtime::{
    DetectionStatus, GameInstallation, LaunchMode, LaunchRequest, RuntimeConfig, RuntimeProvider,
};
use sre_shipwright_adapter::{SHIPWRIGHT_RUNTIME_PROTOCOL, ShipwrightAdapter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const OOT_GAME_ID: &str = "zelda-oot";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameLaunchRequest {
    game_id: String,
    synthetic: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchReport {
    process_id: u32,
    runtime_id: RuntimeId,
    runtime_version: String,
    runtime_protocol: u32,
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn runtime_roots(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let resource_dir = app.path().resource_dir().ok();
    let project_resources = project_root().join("apps/launcher/resources/runtime");
    resource_dir
        .into_iter()
        .flat_map(|directory| {
            [
                directory.join("runtime"),
                directory.join("resources/runtime"),
            ]
        })
        .chain([project_resources])
        .collect()
}

fn adapter(app: &tauri::AppHandle) -> ShipwrightAdapter {
    ShipwrightAdapter::new(runtime_roots(app))
}

pub(crate) fn find_runtime(
    app: &tauri::AppHandle,
) -> Result<Option<sre_runtime::RuntimeInstallation>, String> {
    let detection = adapter(app)
        .detect()
        .map_err(|error| format!("FICSIT-0008: {error}"))?;
    match detection.status {
        DetectionStatus::Available => Ok(detection.installation),
        DetectionStatus::Missing => Ok(None),
        status => Err(format!(
            "FICSIT-0008: Shipwright runtime health is {status:?}: {}",
            detection.summary
        )),
    }
}

fn validate_launch_request(request: &GameLaunchRequest) -> Result<GameId, String> {
    if request.synthetic {
        return Err("FICSIT-0005: Synthetic onboarding can never launch a game.".to_owned());
    }
    let game_id = GameId::new(request.game_id.clone())
        .map_err(|error| format!("FICSIT-0007: Invalid game identifier: {error}"))?;
    if game_id.as_str() != OOT_GAME_ID {
        return Err(format!(
            "FICSIT-0007: {} is not configured in this launcher increment.",
            game_id
        ));
    }
    Ok(game_id)
}

fn session_id() -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "FICSIT-0001: System clock is before the Unix epoch.".to_owned())?
        .as_millis();
    Ok(format!("sre-{timestamp}-{}", std::process::id()))
}

#[tauri::command]
pub(crate) fn launch_game(
    app: tauri::AppHandle,
    request: GameLaunchRequest,
) -> Result<LaunchReport, String> {
    let game_id = validate_launch_request(&request)?;
    crate::services::authorized_account(&app, game_id.as_str(), "nintendo")?;
    let graphics = platform::probe_graphics();
    if !graphics.available {
        return Err(format!(
            "FICSIT-0009: Graphics pre-flight failed: {}",
            graphics.summary
        ));
    }

    let adapter = adapter(&app);
    let runtime = adapter
        .detect()
        .map_err(|error| format!("FICSIT-0008: {error}"))?;
    let runtime_installation = runtime.installation.ok_or_else(|| {
        "FICSIT-0008: The verified Shipwright runtime is not installed.".to_owned()
    })?;
    let runtime_version = runtime_installation
        .version
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());
    let asset_directory = importer::current_asset_directory(&app)?.ok_or_else(|| {
        "FICSIT-0006: No completed imported asset archive is available.".to_owned()
    })?;

    let session = adapter
        .launch(LaunchRequest {
            session_id: session_id()?,
            installation: GameInstallation {
                installation_id: "current".to_owned(),
                game_id,
                variant_id: GameVariantId::new("n64")
                    .expect("the built-in OoT variant id must be valid"),
                runtime_id: RuntimeId::new("shipwright")
                    .expect("the built-in Shipwright runtime id must be valid"),
                root: asset_directory,
                synthetic_fixture: false,
            },
            config: RuntimeConfig::default(),
            mode: LaunchMode::Authorized,
        })
        .map_err(|error| format!("FICSIT-0008: {error}"))?;
    let process_id = session.process_id.ok_or_else(|| {
        "FICSIT-0008: Shipwright did not return a native process identifier.".to_owned()
    })?;

    Ok(LaunchReport {
        process_id,
        runtime_id: session.runtime_id,
        runtime_version,
        runtime_protocol: SHIPWRIGHT_RUNTIME_PROTOCOL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_gate_rejects_synthetic_inactive_and_unconfigured_games() {
        assert!(
            validate_launch_request(&GameLaunchRequest {
                game_id: OOT_GAME_ID.to_owned(),
                synthetic: true,
            })
            .is_err()
        );
        assert!(
            validate_launch_request(&GameLaunchRequest {
                game_id: "zelda-totk".to_owned(),
                synthetic: false,
            })
            .is_err()
        );
        assert_eq!(
            validate_launch_request(&GameLaunchRequest {
                game_id: OOT_GAME_ID.to_owned(),
                synthetic: false,
            })
            .unwrap()
            .as_str(),
            OOT_GAME_ID
        );
    }
}
