mod importer;
mod platform;
mod runtime;
mod storage;

use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use tauri::Manager;

const SUPPORTED_HASHES_JSON: &str = include_str!("../../../../docs/supportedHashes.json");
const TREATY_V2_JSON: &str = include_str!("../../../../packages/treaty/FICSIT-TREATY-0001.v2.json");
const MINIMUM_FREE_BYTES: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct SupportedHash {
    name: String,
    sha1: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationReport {
    pub(crate) file_name: String,
    pub(crate) file_size: u64,
    pub(crate) readable: bool,
    pub(crate) format_recognized: bool,
    pub(crate) format_name: Option<String>,
    pub(crate) version_supported: bool,
    pub(crate) detected_version: Option<String>,
    pub(crate) integrity_validated: bool,
    pub(crate) import_pipeline_available: bool,
    pub(crate) sha1: String,
}

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

fn rom_format(header: [u8; 4]) -> Option<&'static str> {
    match header {
        [0x80, 0x37, 0x12, 0x40] => Some("Big-endian Nintendo 64 ROM"),
        [0x37, 0x80, 0x40, 0x12] => Some("Byte-swapped Nintendo 64 ROM"),
        [0x40, 0x12, 0x37, 0x80] => Some("Little-endian Nintendo 64 ROM"),
        _ => None,
    }
}

pub(crate) fn validate_game_data_sync(path: &Path) -> Result<ValidationReport, String> {
    let file = File::open(path)
        .map_err(|error| format!("FICSIT-0006: Unable to read the selected file: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("FICSIT-0006: Unable to inspect the selected file: {error}"))?;

    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header).map_err(|error| {
        format!("FICSIT-0007: The selected file is too small to be recognized: {error}")
    })?;

    let mut hasher = Sha1::new();
    hasher.update(header);
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("FICSIT-0006: Reading the selected file failed: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = to_hex(&hasher.finalize());
    let supported_hashes: Vec<SupportedHash> = serde_json::from_str(SUPPORTED_HASHES_JSON)
        .map_err(|error| {
            format!("FICSIT-0001: The supported game-data catalog is invalid: {error}")
        })?;
    let supported = supported_hashes
        .iter()
        .find(|entry| entry.sha1.eq_ignore_ascii_case(&digest));
    let format_name = rom_format(header).map(str::to_owned);

    Ok(ValidationReport {
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Selected game data")
            .to_owned(),
        file_size: metadata.len(),
        readable: true,
        format_recognized: format_name.is_some(),
        format_name,
        version_supported: supported.is_some(),
        detected_version: supported.map(|entry| entry.name.clone()),
        integrity_validated: supported.is_some(),
        import_pipeline_available: supported.is_some(),
        sha1: digest,
    })
}

#[tauri::command]
async fn validate_game_data(
    app: tauri::AppHandle,
    path: String,
) -> Result<ValidationReport, String> {
    let mut report =
        tauri::async_runtime::spawn_blocking(move || validate_game_data_sync(Path::new(&path)))
            .await
            .map_err(|error| format!("FICSIT-0001: Validation task failed: {error}"))??;
    report.import_pipeline_available = importer::pipeline_available(&app);
    Ok(report)
}

#[tauri::command]
fn treaty_metadata() -> Result<TreatyMetadata, String> {
    let identity: TreatyIdentity = serde_json::from_str(TREATY_V2_JSON)
        .map_err(|error| format!("FICSIT-0001: The treaty document is invalid: {error}"))?;
    let digest = Sha256::digest(TREATY_V2_JSON.as_bytes());

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
        let test_path = directory.join(format!(".ftep-write-test-{}", std::process::id()));
        fs::create_dir_all(directory)
            .and_then(|_| File::create(&test_path))
            .and_then(|mut file| file.write_all(b"FTEP"))
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
        "FTEP reserves a 5 GiB minimum for import staging and updates.",
    ));

    let runtime_result = runtime::find_runtime(&app);
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
        "Remote services are not mandatory in Milestone 1.5",
        "The launcher remains on its local/synthetic adapter until the control plane milestone.",
    ));

    checks
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(importer::ImportCoordinator::default())
        .invoke_handler(tauri::generate_handler![
            validate_game_data,
            treaty_metadata,
            run_preflight,
            storage::load_onboarding_state,
            storage::save_onboarding_state,
            importer::import_game_data,
            importer::cancel_game_data_import,
            runtime::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running the FTEP launcher");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_all_nintendo_64_byte_orders() {
        assert!(rom_format([0x80, 0x37, 0x12, 0x40]).is_some());
        assert!(rom_format([0x37, 0x80, 0x40, 0x12]).is_some());
        assert!(rom_format([0x40, 0x12, 0x37, 0x80]).is_some());
        assert!(rom_format([0, 0, 0, 0]).is_none());
    }

    #[test]
    fn supported_hash_catalog_is_well_formed() {
        let entries: Vec<SupportedHash> = serde_json::from_str(SUPPORTED_HASHES_JSON).unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|entry| {
            entry.sha1.len() == 40
                && entry
                    .sha1
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        }));
    }

    #[test]
    fn treaty_metadata_matches_the_canonical_document() {
        let metadata = treaty_metadata().unwrap();
        assert_eq!(metadata.treaty_id, "FICSIT-TREATY-0001");
        assert_eq!(metadata.version, "2.0.0");
        assert_eq!(metadata.sha256.len(), 64);
    }
}
