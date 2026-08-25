mod ftep_client;
mod importer;
mod library;
mod platform;
mod runtimes;
mod services;
mod storage;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sre_achievements::AchievementEvent;
use sre_runtime::{DetectionStatus, RuntimeProvider};
use std::fs::{self, File};
use std::io::Write;
use tauri::Manager;

const TREATY_V3_JSON: &str = include_str!("../../../../packages/treaty/FICSIT-ACCORD-0001.v3.json");
const MINIMUM_FREE_BYTES: u64 = 5 * 1024 * 1024 * 1024;
pub(crate) use sre_shipwright_adapter::OotSourceValidation as ValidationReport;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TreatyMetadata {
    treaty_id: String,
    version: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TreatyIdentity {
    treaty_id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreflightRequest {
    synthetic: bool,
    game_data_validated: bool,
    assets_imported: bool,
    authenticated: bool,
    device_registered: bool,
    entitlement_active: bool,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DiagnosticStatus {
    Pass,
    Warning,
    Fail,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticCheck {
    id: String,
    label: String,
    status: DiagnosticStatus,
    summary: String,
    details: String,
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[tauri::command]
async fn validate_game_data(
    app: tauri::AppHandle,
    services: tauri::State<'_, services::LocalServices>,
    path: String,
) -> Result<ValidationReport, String> {
    let adapter = importer::shipwright_adapter(&app);
    let validation = tauri::async_runtime::spawn_blocking(move || {
        adapter
            .validate_oot_source(std::path::Path::new(&path))
            .map_err(|error| format!("FICSIT-0006: {}", error.message))
    })
    .await
    .map_err(|error| format!("FICSIT-0001: Validation task failed: {error}"))?;
    if validation.is_ok() {
        let at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or(0);
        let _ = services::evaluate_cached_achievement(
            &app,
            services.overlay.clone(),
            AchievementEvent::GameDataValidated { at_unix_ms },
        );
    }
    validation
}

#[tauri::command]
fn treaty_metadata() -> Result<TreatyMetadata, String> {
    let identity: TreatyIdentity = serde_json::from_str(TREATY_V3_JSON)
        .map_err(|error| format!("FICSIT-0001: The treaty document is invalid: {error}"))?;
    let canonical: serde_json::Value = serde_json::from_str(TREATY_V3_JSON)
        .map_err(|error| format!("FICSIT-0001: The treaty document is invalid: {error}"))?;
    let canonical = serde_json::to_vec(&canonical)
        .map_err(|error| format!("FICSIT-0001: Cannot canonicalize the Accord: {error}"))?;
    let digest = Sha256::digest(canonical);

    Ok(TreatyMetadata {
        treaty_id: identity.treaty_id,
        version: identity.version,
        sha256: to_hex(&digest),
    })
}

fn check(
    id: &str,
    label: &str,
    status: DiagnosticStatus,
    summary: impl Into<String>,
    details: impl Into<String>,
) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        status,
        summary: summary.into(),
        details: details.into(),
    }
}

#[tauri::command]
fn run_preflight(app: tauri::AppHandle, request: PreflightRequest) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();
    let synthetic_note =
        "Synthetic onboarding proves launcher behavior only and cannot launch Zelda.";

    let os_supported = cfg!(target_os = "windows");
    checks.push(check(
        "os",
        "Operating system",
        if os_supported || request.synthetic {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if os_supported {
            "Supported Windows environment"
        } else if request.synthetic {
            "Accepted for synthetic launcher development"
        } else {
            "This first release requires Windows"
        },
        std::env::consts::OS,
    ));

    let arch_supported = std::env::consts::ARCH == "x86_64";
    checks.push(check(
        "cpu",
        "CPU architecture",
        if arch_supported || request.synthetic {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if arch_supported {
            "64-bit runtime supported"
        } else {
            "Windows x64 is required"
        },
        std::env::consts::ARCH,
    ));

    let app_data_dir = app.path().app_data_dir().ok();
    let writable = app_data_dir.as_ref().is_some_and(|directory| {
        let test_path = directory.join(format!(".sre-write-test-{}", std::process::id()));
        fs::create_dir_all(directory)
            .and_then(|_| File::create(&test_path))
            .and_then(|mut file| file.write_all(b"SRE"))
            .and_then(|_| fs::remove_file(test_path))
            .is_ok()
    });
    checks.push(check(
        "save-directory",
        "Save directory",
        if writable {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if writable {
            "Application data directory is writable"
        } else {
            "Application data directory is not writable"
        },
        app_data_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Application data path unavailable".to_owned()),
    ));

    let free_bytes = app_data_dir
        .as_ref()
        .and_then(|path| fs2::available_space(path).ok());
    let enough_space = free_bytes.is_some_and(|bytes| bytes >= MINIMUM_FREE_BYTES);
    checks.push(check(
        "disk-space",
        "Free disk space",
        if enough_space || request.synthetic {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        free_bytes
            .map(|bytes| format!("{:.1} GiB available", bytes as f64 / 1024_f64.powi(3)))
            .unwrap_or_else(|| "Free space could not be measured".to_owned()),
        "SRE reserves a 5 GiB minimum for import staging and updates.",
    ));

    let runtime_result = importer::shipwright_adapter(&app)
        .detect()
        .map_err(|error| format!("FICSIT-0008: {}", error.message))
        .and_then(|detection| match detection.status {
            DetectionStatus::Available => Ok(detection.installation),
            DetectionStatus::Missing => Ok(None),
            status => Err(format!(
                "FICSIT-0008: Shipwright runtime health is {status:?}: {}",
                detection.summary
            )),
        });
    let runtime = runtime_result.as_ref().ok().and_then(Option::as_ref);
    checks.push(check(
        "runtime",
        "Configured runtime",
        if runtime.is_some() || request.synthetic {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        runtime
            .as_ref()
            .map(|_| "Shipwright provider located")
            .unwrap_or(if request.synthetic {
                "Runtime intentionally omitted from the synthetic path"
            } else {
                "Shipwright provider is missing"
            }),
        runtime
            .and_then(|installation| installation.executable.as_ref())
            .map(|executable| executable.display().to_string())
            .unwrap_or_else(|| {
                runtime_result
                    .err()
                    .unwrap_or_else(|| synthetic_note.to_owned())
            }),
    ));

    let imported_assets_ready = if request.synthetic {
        request.assets_imported
    } else {
        importer::current_asset_directory(&app)
            .ok()
            .flatten()
            .is_some()
    };

    for (id, label, ready, missing_summary) in [
        (
            "game-data",
            "Game data",
            request.game_data_validated,
            "Game data has not been validated",
        ),
        (
            "assets",
            "Imported assets",
            imported_assets_ready,
            "Asset import has not completed",
        ),
        (
            "authentication",
            "Authentication",
            request.authenticated,
            "Account login is required",
        ),
        (
            "device",
            "Device identity",
            request.device_registered,
            "Device registration is required",
        ),
        (
            "entitlement",
            "Treaty entitlement",
            request.entitlement_active,
            "A valid entitlement is required",
        ),
    ] {
        checks.push(check(
            id,
            label,
            if ready {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Fail
            },
            if ready { "Ready" } else { missing_summary },
            if request.synthetic {
                synthetic_note
            } else {
                "Validated by the corresponding onboarding stage."
            },
        ));
    }

    let graphics = platform::probe_graphics();
    checks.push(check(
        "graphics",
        "Graphics",
        if request.synthetic || graphics.available {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Fail
        },
        if request.synthetic {
            "Synthetic diagnostic passed".to_owned()
        } else {
            graphics.summary
        },
        if request.synthetic {
            synthetic_note.to_owned()
        } else {
            graphics.details
        },
    ));

    let controller = platform::probe_controller();
    checks.push(check(
        "controller",
        "Controller",
        if request.synthetic || controller.available {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warning
        },
        if request.synthetic {
            "Synthetic diagnostic passed".to_owned()
        } else {
            controller.summary
        },
        if request.synthetic {
            synthetic_note.to_owned()
        } else {
            controller.details
        },
    ));

    checks.push(check(
        "network",
        "Control plane",
        DiagnosticStatus::Warning,
        "Remote services are optional while a cached lease remains valid",
        "Connect FTEP to refresh account, policy, and entitlement state when available.",
    ));

    checks
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(importer::ImportCoordinator::default())
        .manage(services::LocalServices::default())
        .setup(|app| {
            if let Some(overlay) = app.get_webview_window("overlay") {
                overlay.set_ignore_cursor_events(true)?;
            }
            if let Err(error) = app
                .state::<services::LocalServices>()
                .start_managed_local_ftep(app.handle())
            {
                eprintln!("{error}");
            }
            if let Some(main) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                        app_handle
                            .state::<services::LocalServices>()
                            .shutdown_managed_local_ftep();
                        #[cfg(windows)]
                        std::process::exit(0);
                        #[cfg(not(windows))]
                        app_handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            validate_game_data,
            treaty_metadata,
            run_preflight,
            storage::load_onboarding_state,
            storage::save_onboarding_state,
            importer::import_game_data,
            importer::cancel_game_data_import,
            library::game_catalog,
            library::load_library,
            library::register_game,
            library::register_imported_shipwright_game,
            runtimes::emulator_inventory,
            runtimes::open_emulator,
            runtimes::open_emulator_settings,
            runtimes::open_emulator_runtime_folder,
            services::device_identity,
            services::verify_cached_lease,
            services::cache_entitlement_lease,
            services::entitlement_status,
            services::session_history,
            services::achievement_count,
            services::ftep_web_url,
            services::begin_ftep_connection,
            services::launch_registered_game,
            services::cancel_registered_game_launch,
            services::stop_running_game,
            services::active_running_game_sessions,
            services::run_sre_doctor,
            services::export_diagnostics,
            services::pop_overlay,
            services::hide_overlay
        ])
        .build(tauri::generate_context!())
        .expect("error while running SRE");

    app.run(|app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            app_handle
                .state::<services::LocalServices>()
                .shutdown_managed_local_ftep();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treaty_metadata_matches_the_canonical_document() {
        let metadata = treaty_metadata().unwrap();
        assert_eq!(metadata.treaty_id, "FICSIT-ACCORD-0001");
        assert_eq!(metadata.version, "3.0.0");
        assert_eq!(metadata.sha256.len(), 64);
    }
}
