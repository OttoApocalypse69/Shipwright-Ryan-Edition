use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sre_core::GameCatalog;
use std::fs;
use std::path::{Path, PathBuf};

pub const CATALOG_JSON: &str = include_str!("../../../packages/game-catalog/catalog.v1.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckStatus {
    Pass,
    Warning,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub id: String,
    pub status: CheckStatus,
    pub summary: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub schema_version: u32,
    pub product: String,
    pub generated_at_unix_ms: u64,
    pub platform: String,
    pub checks: Vec<DoctorCheck>,
}

pub fn run_doctor(data_directory: &Path, runtime_executable: Option<&Path>) -> DoctorReport {
    let catalog = GameCatalog::from_json(CATALOG_JSON);
    let accord = ftep_policy::load_accord();
    let storage = storage_check(data_directory);
    let runtime = match runtime_executable {
        Some(path) if path.is_file() => DoctorCheck {
            id: "runtime".to_owned(),
            status: CheckStatus::Pass,
            summary: format!("External runtime selected ({})", path_token(path)),
            remediation: None,
        },
        Some(path) => DoctorCheck {
            id: "runtime".to_owned(),
            status: CheckStatus::Fail,
            summary: format!("Selected runtime is unavailable ({})", path_token(path)),
            remediation: Some("Select the runtime executable again in SRE setup.".to_owned()),
        },
        None => DoctorCheck {
            id: "runtime".to_owned(),
            status: CheckStatus::Warning,
            summary: "No external runtime supplied to this diagnostic run.".to_owned(),
            remediation: Some(
                "Run diagnostics from a configured game for runtime-specific checks.".to_owned(),
            ),
        },
    };
    DoctorReport {
        schema_version: 1,
        product: "SRE".to_owned(),
        generated_at_unix_ms: now_unix_ms(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        checks: vec![
            DoctorCheck {
                id: "catalog".to_owned(),
                status: if catalog.is_ok() {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                summary: catalog
                    .map(|value| format!("{} catalog games validated", value.games.len()))
                    .unwrap_or_else(|error| error.to_string()),
                remediation: None,
            },
            DoctorCheck {
                id: "accord".to_owned(),
                status: if accord.is_ok() {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                summary: accord
                    .map(|value| format!("Accord {} validated", value.version))
                    .unwrap_or_else(|error| error),
                remediation: None,
            },
            storage,
            runtime,
        ],
    }
}

pub fn export_sanitized(report: &DoctorReport, destination: &Path) -> Result<(), String> {
    let parent = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    fs::write(
        destination,
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn storage_check(directory: &Path) -> DoctorCheck {
    let token = path_token(directory);
    let probe = directory.join(format!(".sre-doctor-{}", std::process::id()));
    let result = fs::create_dir_all(directory)
        .and_then(|_| fs::write(&probe, b"probe"))
        .and_then(|_| fs::remove_file(probe));
    match result {
        Ok(()) => DoctorCheck {
            id: "storage".to_owned(),
            status: CheckStatus::Pass,
            summary: format!("Application storage is writable ({token})"),
            remediation: None,
        },
        Err(error) => DoctorCheck {
            id: "storage".to_owned(),
            status: CheckStatus::Fail,
            summary: format!("Application storage is not writable ({token}): {error}"),
            remediation: Some(
                "Choose a writable SRE data directory or repair permissions.".to_owned(),
            ),
        },
    }
}

fn path_token(path: &Path) -> String {
    let normalized = path.to_string_lossy().to_ascii_lowercase();
    let digest = Sha256::digest(normalized.as_bytes());
    format!(
        "path:{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3]
    )
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

pub fn default_data_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("SRE")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_bundle_contains_no_raw_user_path() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("Secret User Name").join("SRE");
        let report = run_doctor(&data, Some(&temp.path().join("private/runtime.exe")));
        let output = serde_json::to_string(&report).unwrap();
        assert!(!output.contains("Secret User Name"));
        assert!(!output.contains("private"));
    }
}
