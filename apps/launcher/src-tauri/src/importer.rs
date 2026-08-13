use crate::storage::{app_data_dir, atomic_write};
use crate::validate_game_data_sync;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const SHIPWRIGHT_VERSION: &str = "9.2.3";
const MINIMUM_IMPORT_SPACE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct ImportCoordinator {
    running: Arc<AtomicBool>,
    cancel_requested: Arc<AtomicBool>,
}

impl Default for ImportCoordinator {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRequest {
    path: String,
    expected_sha1: String,
    detected_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportReport {
    archive_name: String,
    installation_id: String,
    imported_at_unix_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssetManifest {
    schema_version: u32,
    installation_id: String,
    archive_name: String,
    detected_version: String,
    source_sha1: String,
    shipwright_version: String,
    completed_at_unix_ms: u128,
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn first_existing_file(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.is_file())
}

fn first_existing_directory(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.is_dir())
}

pub(crate) fn find_extractor(app: &tauri::AppHandle) -> Option<PathBuf> {
    let resource_dir = app.path().resource_dir().ok();
    let root = project_root();
    first_existing_file(
        resource_dir
            .into_iter()
            .flat_map(|directory| {
                [
                    directory.join("extractor/soh-torch.exe"),
                    directory.join("resources/extractor/soh-torch.exe"),
                ]
            })
            .chain([
                root.join("build-ftep-tools/soh-torch.exe"),
                root.join("build-ftep-tools/Release/soh-torch.exe"),
                root.join("apps/launcher/resources/extractor/soh-torch.exe"),
            ]),
    )
}

fn find_extractor_definitions(app: &tauri::AppHandle) -> Option<PathBuf> {
    let resource_dir = app.path().resource_dir().ok();
    let root = project_root();
    first_existing_directory(
        resource_dir
            .into_iter()
            .flat_map(|directory| {
                [
                    directory.join("extractor/yml"),
                    directory.join("resources/extractor/yml"),
                ]
            })
            .chain([
                root.join("soh/assets/yml"),
                root.join("apps/launcher/resources/extractor/yml"),
            ]),
    )
}

pub(crate) fn pipeline_available(app: &tauri::AppHandle) -> bool {
    find_extractor(app).is_some() && find_extractor_definitions(app).is_some()
}

pub(crate) fn assets_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = app_data_dir(app)?.join("assets");
    fs::create_dir_all(root.join("staging"))
        .and_then(|_| fs::create_dir_all(root.join("installs")))
        .map_err(|error| {
            format!("FICSIT-0006: Asset directories could not be prepared: {error}")
        })?;
    Ok(root)
}

pub(crate) fn load_asset_manifest(app: &tauri::AppHandle) -> Result<Option<AssetManifest>, String> {
    let path = assets_root(app)?.join("current.json");
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| {
        format!("FICSIT-0006: Imported asset metadata could not be read: {error}")
    })?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("FICSIT-0006: Imported asset metadata is invalid: {error}"))
}

pub(crate) fn current_asset_directory(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(manifest) = load_asset_manifest(app)? else {
        return Ok(None);
    };
    let root = assets_root(app)?;
    let directory = root.join("installs").join(&manifest.installation_id);
    let archive = directory.join(&manifest.archive_name);
    if !archive.is_file() {
        return Ok(None);
    }
    Ok(Some(directory))
}

fn import_game_data_sync(
    app: tauri::AppHandle,
    request: ImportRequest,
    coordinator: ImportCoordinator,
) -> Result<ImportReport, String> {
    let report = validate_game_data_sync(Path::new(&request.path))?;
    if !report.version_supported || !report.integrity_validated || !report.import_pipeline_available
    {
        return Err(
            "FICSIT-0007: The supplied game data is not supported by the import pipeline."
                .to_owned(),
        );
    }
    if !report.sha1.eq_ignore_ascii_case(&request.expected_sha1) {
        return Err(
            "FICSIT-0006: The selected file changed after validation. Please validate it again."
                .to_owned(),
        );
    }

    let extractor = find_extractor(&app).ok_or_else(|| {
        "FICSIT-0006: The maintained Shipwright extractor is not installed.".to_owned()
    })?;
    let definitions = find_extractor_definitions(&app).ok_or_else(|| {
        "FICSIT-0006: Shipwright extractor definitions are not installed.".to_owned()
    })?;
    let root = assets_root(&app)?;
    let required_space = MINIMUM_IMPORT_SPACE.max(report.file_size.saturating_mul(8));
    let available_space = fs2::available_space(&root)
        .map_err(|error| format!("FICSIT-0006: Free disk space could not be measured: {error}"))?;
    if available_space < required_space {
        return Err(format!(
            "FICSIT-0006: Asset import requires at least {:.1} GiB free.",
            required_space as f64 / 1024_f64.powi(3)
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix("import-")
        .tempdir_in(root.join("staging"))
        .map_err(|error| format!("FICSIT-0006: Import staging could not be created: {error}"))?;
    let stdout_path = staging.path().join("extractor.stdout.log");
    let stderr_path = staging.path().join("extractor.stderr.log");
    let stdout = File::create(&stdout_path).map_err(|error| {
        format!("FICSIT-0006: Import diagnostics could not be prepared: {error}")
    })?;
    let stderr = File::create(&stderr_path).map_err(|error| {
        format!("FICSIT-0006: Import diagnostics could not be prepared: {error}")
    })?;

    let mut child = Command::new(&extractor)
        .arg("--src")
        .arg(&definitions)
        .arg("--dest")
        .arg(staging.path())
        .arg("--version")
        .arg(SHIPWRIGHT_VERSION)
        .arg(&request.path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("FICSIT-0006: Shipwright extraction could not start: {error}"))?;

    let exit_status = loop {
        if coordinator.cancel_requested.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "FICSIT-0006: Asset import was cancelled. The original file was not changed."
                    .to_owned(),
            );
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                return Err(format!(
                    "FICSIT-0006: Asset import could not be monitored: {error}"
                ));
            }
        }
    };
    if !exit_status.success() {
        return Err(format!(
            "FICSIT-0006: Shipwright extraction failed with exit code {}. Technical logs were discarded with staging data.",
            exit_status.code().unwrap_or(-1)
        ));
    }

    let mut archives = fs::read_dir(staging.path())
        .map_err(|error| format!("FICSIT-0006: Import output could not be inspected: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("o2r"))
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "oot.o2r" || name == "oot-mq.o2r")
        })
        .collect::<Vec<_>>();
    archives.sort();
    let archive = archives.into_iter().next().ok_or_else(|| {
        "FICSIT-0006: Shipwright produced no usable game-data archive.".to_owned()
    })?;
    if archive
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(0)
        == 0
    {
        return Err("FICSIT-0006: Shipwright produced an empty game-data archive.".to_owned());
    }

    let _ = fs::remove_file(staging.path().join("torch.hash.yml"));
    let _ = fs::remove_file(stdout_path);
    let _ = fs::remove_file(stderr_path);

    let imported_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "FICSIT-0001: System clock is before the Unix epoch.".to_owned())?
        .as_millis();
    let installation_id = format!("{}-{}", imported_at_unix_ms, std::process::id());
    let installation_dir = root.join("installs").join(&installation_id);
    fs::rename(staging.path(), &installation_dir)
        .map_err(|error| format!("FICSIT-0006: Completed assets could not be promoted: {error}"))?;

    let archive_name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("oot.o2r")
        .to_owned();
    let manifest = AssetManifest {
        schema_version: 1,
        installation_id: installation_id.clone(),
        archive_name: archive_name.clone(),
        detected_version: request.detected_version,
        source_sha1: report.sha1,
        shipwright_version: SHIPWRIGHT_VERSION.to_owned(),
        completed_at_unix_ms: imported_at_unix_ms,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        format!("FICSIT-0006: Import metadata could not be serialized: {error}")
    })?;
    if let Err(error) = atomic_write(&root.join("current.json"), &manifest_bytes) {
        let _ = fs::remove_dir_all(&installation_dir);
        return Err(error);
    }

    Ok(ImportReport {
        archive_name,
        installation_id,
        imported_at_unix_ms,
    })
}

#[tauri::command]
pub(crate) async fn import_game_data(
    app: tauri::AppHandle,
    coordinator: tauri::State<'_, ImportCoordinator>,
    request: ImportRequest,
) -> Result<ImportReport, String> {
    let coordinator = coordinator.inner().clone();
    if coordinator.running.swap(true, Ordering::SeqCst) {
        return Err("FICSIT-0006: An asset import is already running.".to_owned());
    }
    coordinator.cancel_requested.store(false, Ordering::SeqCst);
    let worker_coordinator = coordinator.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        import_game_data_sync(app, request, worker_coordinator)
    })
    .await
    .map_err(|error| format!("FICSIT-0001: Import task failed: {error}"))?;
    coordinator.running.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
pub(crate) fn cancel_game_data_import(coordinator: tauri::State<'_, ImportCoordinator>) -> bool {
    let running = coordinator.running.load(Ordering::SeqCst);
    if running {
        coordinator.cancel_requested.store(true, Ordering::SeqCst);
    }
    running
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_contains_shipwright_sources() {
        assert!(project_root().join("soh/assets/yml").is_dir());
        assert!(project_root().join("docs/supportedHashes.json").is_file());
    }
}
