use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::Manager;

const MAX_STATE_BYTES: usize = 64 * 1024;

pub(crate) fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_data_dir().map_err(|error| {
        format!("FICSIT-0001: Application data directory is unavailable: {error}")
    })?;
    fs::create_dir_all(&directory).map_err(|error| {
        format!("FICSIT-0001: Application data directory could not be created: {error}")
    })?;
    Ok(directory)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "FICSIT-0001: Storage path has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("FICSIT-0001: Storage directory could not be created: {error}"))?;

    let mut temporary = tempfile::Builder::new()
        .prefix(".sre-write-")
        .tempfile_in(parent)
        .map_err(|error| {
            format!("FICSIT-0001: Temporary state file could not be created: {error}")
        })?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("FICSIT-0001: State could not be written safely: {error}"))?;
    temporary.persist(path).map_err(|error| {
        format!(
            "FICSIT-0001: State could not be promoted safely: {}",
            error.error
        )
    })?;
    Ok(())
}

fn onboarding_state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("onboarding-state.json"))
}

#[tauri::command]
pub(crate) fn load_onboarding_state(app: tauri::AppHandle) -> Result<Option<Value>, String> {
    let path = onboarding_state_path(&app)?;
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path).map_err(|error| {
        format!("FICSIT-0001: Saved onboarding state could not be read: {error}")
    })?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("FICSIT-0001: Saved onboarding state exceeded the safety limit.".to_owned());
    }

    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("FICSIT-0001: Saved onboarding state is invalid: {error}"))?;
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Ok(None);
    }
    Ok(Some(value))
}

#[tauri::command]
pub(crate) fn save_onboarding_state(app: tauri::AppHandle, state: Value) -> Result<(), String> {
    if state.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(
            "FICSIT-0001: Refusing to save an unknown onboarding-state version.".to_owned(),
        );
    }
    let bytes = serde_json::to_vec_pretty(&state).map_err(|error| {
        format!("FICSIT-0001: Onboarding state could not be serialized: {error}")
    })?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("FICSIT-0001: Onboarding state exceeded the safety limit.".to_owned());
    }
    atomic_write(&onboarding_state_path(&app)?, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_existing_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"second");
    }
}
