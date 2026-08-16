use crate::{importer, storage};
use serde::{Deserialize, Serialize};
use sre_runtime::RuntimeProvider;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TWO_SHIP_RUNTIME_VERSION: &str = "5.0.0";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmulatorInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub version: String,
    pub status: String,
    pub managed: bool,
    pub executable_path: Option<String>,
    pub runtime_directory: Option<String>,
    pub settings_directory: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmulatorRequest {
    pub runtime_id: String,
}

struct RuntimeTarget {
    id: &'static str,
    name: &'static str,
    platform: &'static str,
    version: &'static str,
    executable: Option<PathBuf>,
    runtime_directory: PathBuf,
    settings_directory: PathBuf,
    description: &'static str,
    launch_arguments: &'static [&'static str],
    managed: bool,
}

pub(crate) fn inventory(app: &tauri::AppHandle) -> Result<Vec<EmulatorInfo>, String> {
    [
        "shipwright",
        "two-ship",
        "cemu",
        "ryujinx-canary",
        "dolphin-compatible",
    ]
    .into_iter()
    .map(|runtime_id| {
        let target = runtime_target(app, runtime_id)?;
        let installed = target
            .executable
            .as_ref()
            .is_some_and(|path| path.is_file());
        Ok(EmulatorInfo {
            id: target.id.to_owned(),
            name: target.name.to_owned(),
            platform: target.platform.to_owned(),
            version: target.version.to_owned(),
            status: if installed { "READY" } else { "MISSING" }.to_owned(),
            managed: target.managed,
            executable_path: target
                .executable
                .as_ref()
                .map(|path| path.display().to_string()),
            runtime_directory: target
                .executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(|path| path.display().to_string()),
            settings_directory: target.settings_directory.display().to_string(),
            description: target.description.to_owned(),
        })
    })
    .collect()
}

#[tauri::command]
pub(crate) fn emulator_inventory(app: tauri::AppHandle) -> Result<Vec<EmulatorInfo>, String> {
    inventory(&app)
}

#[tauri::command]
pub(crate) fn open_emulator(app: tauri::AppHandle, request: EmulatorRequest) -> Result<(), String> {
    let target = runtime_target_for_action(&app, &request.runtime_id)?;
    let executable = target.executable.ok_or_else(|| {
        format!(
            "FICSIT-0008: {} is not installed or could not be located.",
            target.name
        )
    })?;
    if !executable.is_file() {
        return Err(format!(
            "FICSIT-0008: {} is missing at {}.",
            target.name,
            executable.display()
        ));
    }

    let mut command = Command::new(&executable);
    command
        .args(target.launch_arguments)
        .current_dir(&target.runtime_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.spawn().map_err(|error| {
        format!(
            "FICSIT-0008: Could not open {} from {}: {error}",
            target.name,
            executable.display()
        )
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn open_emulator_settings(
    app: tauri::AppHandle,
    request: EmulatorRequest,
) -> Result<(), String> {
    let target = runtime_target_for_action(&app, &request.runtime_id)?;
    std::fs::create_dir_all(&target.settings_directory).map_err(|error| {
        format!(
            "FICSIT-0008: Could not prepare the {} settings folder at {}: {error}",
            target.name,
            target.settings_directory.display()
        )
    })?;
    open_directory(&target.settings_directory)
}

#[tauri::command]
pub(crate) fn open_emulator_runtime_folder(
    app: tauri::AppHandle,
    request: EmulatorRequest,
) -> Result<(), String> {
    let target = runtime_target_for_action(&app, &request.runtime_id)?;
    if !target.runtime_directory.is_dir() {
        return Err(format!(
            "FICSIT-0008: The {} runtime folder is missing at {}.",
            target.name,
            target.runtime_directory.display()
        ));
    }
    open_directory(&target.runtime_directory)
}

fn runtime_target_for_action(
    app: &tauri::AppHandle,
    runtime_id: &str,
) -> Result<RuntimeTarget, String> {
    if runtime_id == "two-ship" {
        let app_data = storage::app_data_dir(app)?;
        let staged_executable = app_data
            .join("runtimes")
            .join(format!("two-ship-{TWO_SHIP_RUNTIME_VERSION}"))
            .join("2ship.exe");
        if !staged_executable.is_file() {
            importer::managed_two_ship_executable(app)?;
        }
    }
    runtime_target(app, runtime_id)
}

/// Resolve the roaming profile used by the emulator itself. Managed runtime
/// binaries are owned by SRE, but their keys, firmware, and settings must stay
/// in the standard per-user locations that the emulators already read.
fn roaming_settings_directory(
    app: &tauri::AppHandle,
    emulator_directory: &str,
) -> Result<PathBuf, String> {
    if let Some(roaming) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(roaming).join(emulator_directory));
    }
    Ok(storage::app_data_dir(app)?.join(emulator_directory))
}

fn runtime_target(app: &tauri::AppHandle, runtime_id: &str) -> Result<RuntimeTarget, String> {
    let app_data = storage::app_data_dir(app)?;
    match runtime_id {
        "shipwright" => {
            let executable = importer::shipwright_adapter(app)
                .detect()
                .ok()
                .and_then(|detection| detection.installation)
                .and_then(|installation| installation.executable);
            let runtime_directory = executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| app_data.join("assets"));
            Ok(RuntimeTarget {
                id: "shipwright",
                name: "Shipwright",
                platform: "Nintendo 64",
                version: "9.2.3",
                executable,
                runtime_directory: runtime_directory.clone(),
                settings_directory: runtime_directory,
                description: "Native Ocarina of Time runtime and its local settings.",
                launch_arguments: &[],
                managed: true,
            })
        }
        "two-ship" => {
            let staged = app_data
                .join("runtimes")
                .join(format!("two-ship-{TWO_SHIP_RUNTIME_VERSION}"))
                .join("2ship.exe");
            let bundled = importer::two_ship_executable(app).ok();
            let executable = staged.is_file().then_some(staged).or(bundled);
            let runtime_directory = executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| app_data.join("runtimes"));
            Ok(RuntimeTarget {
                id: "two-ship",
                name: "2 Ship 2 Harkinian",
                platform: "Nintendo 64",
                version: TWO_SHIP_RUNTIME_VERSION,
                executable,
                runtime_directory: runtime_directory.clone(),
                settings_directory: roaming_settings_directory(app, "2Ship2Harkinian")?,
                description: "Managed Majora's Mask runtime with writable local settings.",
                launch_arguments: &[],
                managed: true,
            })
        }
        "cemu" => {
            let executable = importer::managed_cemu_executable(app).ok();
            let runtime_directory = executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| app_data.join("runtimes").join("cemu-2.6"));
            Ok(RuntimeTarget {
                id: "cemu",
                name: "Cemu",
                platform: "Wii U",
                version: "2.6",
                executable,
                runtime_directory,
                settings_directory: roaming_settings_directory(app, "Cemu")?,
                description: "Managed Cemu runtime for Breath of the Wild and Wii U settings.",
                launch_arguments: &[],
                managed: true,
            })
        }
        "ryujinx-canary" => {
            let executable = importer::managed_ryujinx_executable(app).ok();
            let runtime_directory = executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| app_data.join("runtimes").join("ryujinx-canary"));
            Ok(RuntimeTarget {
                id: "ryujinx-canary",
                name: "Ryujinx Canary",
                platform: "Nintendo Switch",
                version: "1.3.340",
                executable,
                runtime_directory,
                settings_directory: roaming_settings_directory(app, "Ryujinx")?,
                description: "Managed Switch runtime for Tears of the Kingdom, Echoes of Wisdom, and Animal Crossing.",
                // Canary's retired update endpoint can block startup, so keep
                // the same safe flag used by managed game launches.
                launch_arguments: &["--hide-updates"],
                managed: true,
            })
        }
        "dolphin-compatible" => {
            let executable = importer::detected_dolphin_executable();
            let runtime_directory = executable
                .as_ref()
                .and_then(|path| path.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| app_data.join("runtimes/dolphin"));
            Ok(RuntimeTarget {
                id: "dolphin-compatible",
                name: "Dolphin",
                platform: "Wii",
                version: "External",
                executable,
                runtime_directory,
                settings_directory: roaming_settings_directory(app, "Dolphin Emulator")?,
                description: "User-selected Dolphin runtime for Wii game images.",
                launch_arguments: &[],
                managed: false,
            })
        }
        _ => Err(format!(
            "FICSIT-0008: Unknown emulator runtime '{runtime_id}'."
        )),
    }
}

#[cfg(windows)]
fn open_directory(path: &std::path::Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|error| format!("FICSIT-0008: Could not open {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(not(windows))]
fn open_directory(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let opener = "xdg-open";
    Command::new(opener)
        .arg(path)
        .spawn()
        .map_err(|error| format!("FICSIT-0008: Could not open {}: {error}", path.display()))?;
    Ok(())
}
