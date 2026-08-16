use crate::storage::app_data_dir;
use serde::Deserialize;
use sre_shipwright_adapter::{OotImportReport, OotImportRequest, ShipwrightAdapter};
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use tauri::Manager;

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

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Application paths are supplied as opaque roots. The Shipwright adapter owns
/// every executable, definition, source-hash, archive, and manifest detail.
pub(crate) fn shipwright_adapter(app: &tauri::AppHandle) -> ShipwrightAdapter {
    let resource_dir = app.path().resource_dir().ok();
    let project_root = project_root();
    let runtime_roots = resource_dir
        .iter()
        .flat_map(|directory| {
            [
                directory.join("runtime"),
                directory.join("resources/runtime"),
            ]
        })
        .chain([project_root.join("apps/launcher/resources/runtime")])
        .collect::<Vec<_>>();
    let integration_roots = resource_dir.into_iter().chain([project_root]);
    ShipwrightAdapter::new(runtime_roots).with_integration_roots(integration_roots)
}

/// Returns the application-owned 2 Ship executable without exposing bundled
/// resource layout to setup or session code.
pub(crate) fn two_ship_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let project_root = project_root();
    let candidates = app
        .path()
        .resource_dir()
        .ok()
        .into_iter()
        .flat_map(|directory| {
            [
                directory.join("runtime/two-ship/2ship.exe"),
                directory.join("resources/runtime/two-ship/2ship.exe"),
            ]
        })
        .chain([project_root.join("apps/launcher/resources/runtime/two-ship/2ship.exe")]);
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            "FICSIT-0008: SRE's bundled Majora's Mask runtime is missing. Reinstall SRE.".to_owned()
        })
}

const TWO_SHIP_RUNTIME_VERSION: &str = "5.0.0";

// Ryujinx creates a new text log for each process and can emit an enormous
// amount of repeated output when a title is stuck during boot. Keep recent
// logs useful for diagnostics, but never let stale logs consume the disk.
const RYUJINX_LOG_KEEP_COUNT: usize = 5;
const RYUJINX_LOG_MAX_BYTES: u64 = 64 * 1024 * 1024;
const RYUJINX_LOG_ACTIVE_WINDOW: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RyujinxLogFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

fn ryujinx_logs_to_remove(mut logs: Vec<RyujinxLogFile>, now: SystemTime) -> Vec<PathBuf> {
    logs.sort_by_key(|log| Reverse(log.modified));

    // Never touch a log that may belong to a currently running emulator. For
    // older logs, retain only the newest few files while staying below the
    // total budget. An oversized stale log is deliberately removed even when
    // it is one of the newest files; this catches runaway boot logs.
    let mut retained_bytes = 0_u64;
    let mut retained_old_logs = 0_usize;
    let mut removals = Vec::new();
    for log in logs {
        let active = now
            .duration_since(log.modified)
            .map(|age| age < RYUJINX_LOG_ACTIVE_WINDOW)
            .unwrap_or(true);
        if active {
            retained_bytes = retained_bytes.saturating_add(log.size);
            continue;
        }

        let can_retain = retained_old_logs < RYUJINX_LOG_KEEP_COUNT
            && log.size <= RYUJINX_LOG_MAX_BYTES
            && retained_bytes.saturating_add(log.size) <= RYUJINX_LOG_MAX_BYTES;
        if can_retain {
            retained_old_logs += 1;
            retained_bytes = retained_bytes.saturating_add(log.size);
        } else {
            removals.push(log.path);
        }
    }
    removals
}

fn prune_ryujinx_logs(log_directory: &Path) {
    let now = SystemTime::now();
    let logs = fs::read_dir(log_directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_log = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("log"));
            if !is_log {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some(RyujinxLogFile {
                path,
                size: metadata.len(),
                modified: metadata.modified().ok()?,
            })
        })
        .collect::<Vec<_>>();

    for path in ryujinx_logs_to_remove(logs, now) {
        // Log cleanup is best-effort. A file can be locked by a still-running
        // Ryujinx process; that must never prevent the game from launching.
        let _ = fs::remove_file(path);
    }
}

fn copy_two_ship_tree(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!("FICSIT-0008: Could not prepare the managed Majora runtime: {error}")
    })?;
    for entry in fs::read_dir(source).map_err(|error| {
        format!("FICSIT-0008: Could not read the bundled Majora runtime: {error}")
    })? {
        let entry = entry.map_err(|error| {
            format!("FICSIT-0008: Could not enumerate the bundled Majora runtime: {error}")
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| {
                format!("FICSIT-0008: Could not inspect a bundled Majora runtime file: {error}")
            })?
            .is_dir()
        {
            copy_two_ship_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!("FICSIT-0008: Could not stage the bundled Majora runtime: {error}")
            })?;
        }
    }
    Ok(())
}

fn staged_two_ship_executable(
    app: &tauri::AppHandle,
    bundled_executable: PathBuf,
) -> Result<PathBuf, String> {
    let source_root = bundled_executable.parent().ok_or_else(|| {
        "FICSIT-0008: The bundled Majora runtime has no containing folder.".to_owned()
    })?;
    let destination_root = app_data_dir(app)?
        .join("runtimes")
        .join(format!("two-ship-{TWO_SHIP_RUNTIME_VERSION}"));
    let destination_executable = destination_root.join("2ship.exe");
    let marker = destination_root.join(".sre-runtime-version");
    if destination_executable.is_file()
        && destination_root.join("assets").is_dir()
        && fs::read_to_string(&marker).ok().as_deref() == Some(TWO_SHIP_RUNTIME_VERSION)
    {
        return Ok(destination_executable);
    }
    let parent = destination_root
        .parent()
        .ok_or_else(|| "FICSIT-0008: The managed runtime location is invalid.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("FICSIT-0008: Could not prepare SRE runtime storage: {error}"))?;
    let staged_root = parent.join(format!(
        ".two-ship-{TWO_SHIP_RUNTIME_VERSION}-staging-{}",
        uuid::Uuid::new_v4()
    ));
    copy_two_ship_tree(source_root, &staged_root)?;
    fs::write(
        staged_root.join(".sre-runtime-version"),
        TWO_SHIP_RUNTIME_VERSION,
    )
    .map_err(|error| format!("FICSIT-0008: Could not mark the managed Majora runtime: {error}"))?;
    if destination_root.exists() {
        return Err("FICSIT-0008: A previous Majora runtime staging attempt is incomplete. Close SRE and remove only its runtime folder before trying again.".to_owned());
    }
    fs::rename(&staged_root, &destination_root).map_err(|error| {
        format!("FICSIT-0008: Could not finish staging the managed Majora runtime: {error}")
    })?;
    Ok(destination_executable)
}

pub(crate) fn two_ship_adapter(
    app: &tauri::AppHandle,
) -> Result<sre_two_ship_adapter::TwoShipAdapter, String> {
    Ok(sre_two_ship_adapter::TwoShipAdapter::new(
        managed_two_ship_executable(app)?,
    ))
}

/// Returns a writable, application-owned 2 Ship runtime. The first call
/// stages the bundled runtime into app data so settings and generated files do
/// not land beside read-only packaged resources.
pub(crate) fn managed_two_ship_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let bundled_executable = two_ship_executable(app)?;
    staged_two_ship_executable(app, bundled_executable)
}

/// Cemu is an explicitly user-installed managed runtime, never a source of
/// game data, console keys, or firmware.
pub(crate) fn managed_cemu_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let executable = app_data_dir(app)?.join("runtimes/cemu-2.6/Cemu.exe");
    executable.is_file().then_some(executable).ok_or_else(|| {
        "FICSIT-0008: The managed Cemu runtime is missing. Reinstall it from SRE runtime setup."
            .to_owned()
    })
}

fn managed_ryujinx_resource_executable(
    app: &tauri::AppHandle,
    resource_directory: &str,
    missing_message: &str,
) -> Result<PathBuf, String> {
    // During local development Tauri's resource directory is `target/debug`.
    // That directory can retain a runtime staged by an older build, so prefer
    // the checked-in package first. Packaged builds fall back to their normal
    // resource directory because the workspace path does not exist there.
    std::iter::once(project_root().join(format!(
        "apps/launcher/resources/runtime/{resource_directory}/publish/Ryujinx.exe"
    )))
    .chain(
        app.path()
            .resource_dir()
            .ok()
            .into_iter()
            .flat_map(|directory| {
                [
                    directory.join(format!("runtime/{resource_directory}/publish/Ryujinx.exe")),
                    directory.join(format!(
                        "resources/runtime/{resource_directory}/publish/Ryujinx.exe"
                    )),
                ]
            }),
    )
    .find(|candidate| candidate.is_file())
    .map(|executable| {
        if let Some(publish_directory) = executable.parent() {
            prune_ryujinx_logs(&publish_directory.join("Logs"));
        }
        executable
    })
    .ok_or_else(|| missing_message.to_owned())
}

/// Returns the application-owned Canary Ryujinx runtime without exposing
/// bundled resource layout to the library or session layers.
pub(crate) fn managed_ryujinx_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    managed_ryujinx_resource_executable(
        app,
        "ryujinx-canary",
        "FICSIT-0008: SRE's bundled Ryujinx Canary runtime is missing. Reinstall SRE.",
    )
}

/// Detect a user-installed Dolphin executable without downloading or staging
/// anything. Game setup still allows an explicit executable selection when a
/// portable Dolphin build lives elsewhere.
pub(crate) fn detected_dolphin_executable() -> Option<PathBuf> {
    std::env::var_os("DOLPHIN_EXECUTABLE")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .map(|root| root.join("Dolphin Emulator/Dolphin.exe")),
        )
        .chain(
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .map(|root| root.join("Dolphin Emulator/Dolphin.exe")),
        )
        .find(|candidate| candidate.is_file())
}

pub(crate) fn assets_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = app_data_dir(app)?.join("assets");
    fs::create_dir_all(&root)
        .map_err(|error| format!("FICSIT-0006: Asset directory could not be prepared: {error}"))?;
    Ok(root)
}

pub(crate) fn current_asset_directory(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    shipwright_adapter(app)
        .current_oot_asset_directory(&assets_root(app)?)
        .map_err(|error| format!("FICSIT-0006: {}", error.message))
}

#[tauri::command]
pub(crate) async fn import_game_data(
    app: tauri::AppHandle,
    coordinator: tauri::State<'_, ImportCoordinator>,
    request: ImportRequest,
) -> Result<OotImportReport, String> {
    let coordinator = coordinator.inner().clone();
    let assets_root = assets_root(&app)?;
    if coordinator.running.swap(true, Ordering::SeqCst) {
        return Err("FICSIT-0006: An asset import is already running.".to_owned());
    }
    coordinator.cancel_requested.store(false, Ordering::SeqCst);
    let adapter = shipwright_adapter(&app);
    let cancellation = coordinator.cancel_requested.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        adapter.import_oot_assets(
            &OotImportRequest {
                path: PathBuf::from(request.path),
                expected_sha1: request.expected_sha1,
                detected_version: request.detected_version,
            },
            &assets_root,
            &cancellation,
        )
    })
    .await
    .map_err(|error| format!("FICSIT-0001: Import task failed: {error}"))?;
    coordinator.running.store(false, Ordering::SeqCst);
    result.map_err(|error| format!("FICSIT-0006: {}", error.message))
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
    fn stale_ryujinx_logs_are_bounded_without_touching_recent_logs() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let stale = now - Duration::from_secs(RYUJINX_LOG_ACTIVE_WINDOW.as_secs() + 1);
        let recent = now - Duration::from_secs(10);
        let removals = ryujinx_logs_to_remove(
            vec![
                RyujinxLogFile {
                    path: PathBuf::from("runaway.log"),
                    size: RYUJINX_LOG_MAX_BYTES + 1,
                    modified: stale,
                },
                RyujinxLogFile {
                    path: PathBuf::from("recent.log"),
                    size: RYUJINX_LOG_MAX_BYTES + 1,
                    modified: recent,
                },
            ],
            now,
        );

        assert_eq!(removals, vec![PathBuf::from("runaway.log")]);
    }

    #[test]
    fn project_root_contains_shipwright_sources() {
        assert!(project_root().join("soh/assets/yml").is_dir());
        assert!(project_root().join("docs/supportedHashes.json").is_file());
    }
}
