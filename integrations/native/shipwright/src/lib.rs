//! SRE adapter for the Shipwright native runtime.

use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sre_core::{GameDefinition, RuntimeId};
use sre_runtime::{
    DetectionResult, DetectionStatus, DiagnosticSeverity, DistributionMode, GameInstallation,
    GameSource, LaunchMode, LaunchRequest, PlayableDetection, PlayablePrecision,
    PreparationContext, PreparedGame, ProcessObservation, RuntimeCapabilities, RuntimeConfig,
    RuntimeDiagnostic, RuntimeError, RuntimeErrorCode, RuntimeInstallation, RuntimeKind,
    RuntimeProvider, RuntimeSession, RuntimeSessionState, SaveLocation, SourceValidation,
    VerificationResult, VersionStatus, VersionValidation,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SHIPWRIGHT_RUNTIME_PROTOCOL: u32 = 1;
const SHIPWRIGHT_RUNTIME_ID: &str = "shipwright";
const SHIPWRIGHT_PLATFORM: &str = "windows-x64";
const SHIPWRIGHT_VERSION: &str = "9.2.3";
const MINIMUM_IMPORT_SPACE: u64 = 2 * 1024 * 1024 * 1024;
const SUPPORTED_HASHES_JSON: &str = include_str!("../../../../docs/supportedHashes.json");

#[derive(Debug, Deserialize)]
struct SupportedHash {
    name: String,
    sha1: String,
}

/// User-facing result of validating OoT data for the Shipwright importer.
/// This remains adapter-owned because the format, hashes, and variants are
/// Shipwright integration details rather than SRE domain data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OotSourceValidation {
    pub file_name: String,
    pub file_size: u64,
    pub readable: bool,
    pub format_recognized: bool,
    pub format_name: Option<String>,
    pub version_supported: bool,
    pub detected_version: Option<String>,
    pub integrity_validated: bool,
    pub import_pipeline_available: bool,
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OotAssetManifest {
    pub schema_version: u32,
    pub installation_id: String,
    pub archive_name: String,
    pub detected_version: String,
    pub source_sha1: String,
    pub shipwright_version: String,
    pub completed_at_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct OotImportRequest {
    pub path: PathBuf,
    pub expected_sha1: String,
    pub detected_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OotImportReport {
    pub archive_name: String,
    pub installation_id: String,
    pub imported_at_unix_ms: u128,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeMetadata {
    schema_version: u32,
    shipwright_version: String,
    runtime_protocol: u32,
    platform: String,
}

#[derive(Debug, Clone)]
struct RuntimeBundle {
    root: PathBuf,
    executable: PathBuf,
    resource_archive: PathBuf,
    metadata: RuntimeMetadata,
}

pub struct ShipwrightAdapter {
    id: RuntimeId,
    runtime_roots: Vec<PathBuf>,
    integration_roots: Vec<PathBuf>,
    children: Mutex<BTreeMap<u32, Child>>,
}

impl ShipwrightAdapter {
    pub fn new(runtime_roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            id: RuntimeId::new(SHIPWRIGHT_RUNTIME_ID)
                .expect("the built-in Shipwright runtime id must be valid"),
            runtime_roots: runtime_roots.into_iter().collect(),
            integration_roots: Vec::new(),
            children: Mutex::new(BTreeMap::new()),
        }
    }

    /// Supplies application-owned resource roots without leaking their layout
    /// into the provider API. The adapter owns the Shipwright-specific paths
    /// below each root.
    pub fn with_integration_roots(mut self, roots: impl IntoIterator<Item = PathBuf>) -> Self {
        self.integration_roots = roots.into_iter().collect();
        self
    }

    pub const fn runtime_protocol(&self) -> u32 {
        SHIPWRIGHT_RUNTIME_PROTOCOL
    }

    fn inspect_runtime_root(&self, root: &Path) -> Result<Option<RuntimeBundle>, RuntimeError> {
        let executable = root.join("soh.exe");
        let resource_archive = root.join("soh.o2r");
        let metadata_path = root.join("sre-runtime.json");
        if !executable.is_file() || !resource_archive.is_file() || !metadata_path.is_file() {
            return Ok(None);
        }

        let bytes = fs::read(&metadata_path).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime metadata could not be read.",
            )
            .with_technical_details(error.to_string())
        })?;
        let metadata: RuntimeMetadata = serde_json::from_slice(&bytes).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime metadata is invalid.",
            )
            .with_technical_details(error.to_string())
        })?;
        if metadata.schema_version != 1
            || metadata.runtime_protocol != SHIPWRIGHT_RUNTIME_PROTOCOL
            || metadata.platform != SHIPWRIGHT_PLATFORM
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeVersionUnsupported,
                "The installed Shipwright runtime protocol or platform is incompatible.",
            )
            .with_technical_details(format!(
                "schema={}, protocol={}, platform={}",
                metadata.schema_version, metadata.runtime_protocol, metadata.platform
            )));
        }

        Ok(Some(RuntimeBundle {
            root: root.to_owned(),
            executable,
            resource_archive,
            metadata,
        }))
    }

    fn find_bundle(&self) -> Result<Option<RuntimeBundle>, RuntimeError> {
        for root in &self.runtime_roots {
            if let Some(bundle) = self.inspect_runtime_root(root)? {
                return Ok(Some(bundle));
            }
        }
        Ok(None)
    }

    fn cancellation_error() -> RuntimeError {
        RuntimeError::new(
            RuntimeErrorCode::OperationCancelled,
            "Game launch was cancelled before Shipwright started.",
        )
    }

    fn ensure_not_cancelled(cancel_requested: &AtomicBool) -> Result<(), RuntimeError> {
        if cancel_requested.load(Ordering::SeqCst) {
            return Err(Self::cancellation_error());
        }
        Ok(())
    }

    fn files_match(source: &Path, destination: &Path) -> bool {
        source.is_file()
            && destination.is_file()
            && source.metadata().ok().map(|metadata| metadata.len())
                == destination.metadata().ok().map(|metadata| metadata.len())
    }

    fn copy_file_atomically_cancellable(
        source: &Path,
        destination: &Path,
        cancel_requested: &AtomicBool,
    ) -> Result<(), RuntimeError> {
        Self::ensure_not_cancelled(cancel_requested)?;
        let parent = destination.parent().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource destination has no parent directory.",
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource directory could not be prepared.",
            )
            .with_technical_details(error.to_string())
        })?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".ftep-runtime-")
            .tempfile_in(parent)
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Runtime resource staging could not be created.",
                )
                .with_technical_details(error.to_string())
            })?;
        let mut input = fs::File::open(source).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource archive could not be opened.",
            )
            .with_technical_details(error.to_string())
        })?;
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            Self::ensure_not_cancelled(cancel_requested)?;
            let copied = input.read(&mut buffer).map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Runtime resource archive could not be read safely.",
                )
                .with_technical_details(error.to_string())
            })?;
            if copied == 0 {
                break;
            }
            temporary
                .as_file_mut()
                .write_all(&buffer[..copied])
                .map_err(|error| {
                    RuntimeError::new(
                        RuntimeErrorCode::RuntimeConfigurationInvalid,
                        "Runtime resource archive could not be staged safely.",
                    )
                    .with_technical_details(error.to_string())
                })?;
        }
        Self::ensure_not_cancelled(cancel_requested)?;
        temporary
            .as_file_mut()
            .flush()
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Runtime resource archive could not be staged safely.",
                )
                .with_technical_details(error.to_string())
            })?;
        temporary.persist(destination).map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Runtime resource archive could not be promoted safely.",
            )
            .with_technical_details(error.error.to_string())
        })?;
        Ok(())
    }

    fn copy_directory_contents_cancellable(
        source: &Path,
        destination: &Path,
        cancel_requested: &AtomicBool,
    ) -> Result<(), RuntimeError> {
        Self::ensure_not_cancelled(cancel_requested)?;
        fs::create_dir_all(destination).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime assets could not be staged.",
                error.to_string(),
            )
        })?;
        let entries = fs::read_dir(source).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime assets could not be read.",
                error.to_string(),
            )
        })?;
        for entry in entries {
            Self::ensure_not_cancelled(cancel_requested)?;
            let entry = entry.map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Shipwright runtime assets could not be enumerated.",
                    error.to_string(),
                )
            })?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let file_type = entry.file_type().map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Shipwright runtime asset type could not be inspected.",
                    error.to_string(),
                )
            })?;
            if file_type.is_dir() {
                Self::copy_directory_contents_cancellable(
                    &source_path,
                    &destination_path,
                    cancel_requested,
                )?;
            } else if file_type.is_file() {
                Self::copy_file_atomically_cancellable(
                    &source_path,
                    &destination_path,
                    cancel_requested,
                )?;
            } else {
                return Err(Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Shipwright runtime assets contain an unsupported filesystem entry.",
                    source_path.display().to_string(),
                ));
            }
        }
        Ok(())
    }

    fn stage_runtime_assets_cancellable(
        &self,
        bundle: &RuntimeBundle,
        installation_root: &Path,
        cancel_requested: &AtomicBool,
    ) -> Result<(), RuntimeError> {
        Self::ensure_not_cancelled(cancel_requested)?;
        let source = self.extractor_assets().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright's bundled extractor assets are missing.",
            )
        })?;
        let marker = installation_root.join(".sre-shipwright-assets-version");
        let version = format!("{}\n", bundle.metadata.shipwright_version);
        let destination = installation_root.join("assets");
        if destination.is_dir() && fs::read_to_string(&marker).ok().as_deref() == Some(&version) {
            return Ok(());
        }

        fs::create_dir_all(installation_root).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright installation directory could not be prepared.",
                error.to_string(),
            )
        })?;
        let staging = tempfile::Builder::new()
            .prefix(".sre-shipwright-assets-")
            .tempdir_in(installation_root)
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Shipwright runtime asset staging could not be created.",
                    error.to_string(),
                )
            })?;
        let staged_assets = staging.path().join("assets");
        Self::copy_directory_contents_cancellable(&source, &staged_assets, cancel_requested)?;

        Self::ensure_not_cancelled(cancel_requested)?;
        if destination.exists() {
            fs::remove_dir_all(&destination).map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "An incomplete Shipwright runtime asset directory could not be replaced.",
                    error.to_string(),
                )
            })?;
        }
        fs::rename(&staged_assets, &destination).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright runtime assets could not be promoted safely.",
                error.to_string(),
            )
        })?;
        Self::atomic_write(&marker, version.as_bytes())
    }

    fn stage_runtime_bundle_cancellable(
        &self,
        bundle: &RuntimeBundle,
        installation_root: &Path,
        cancel_requested: &AtomicBool,
    ) -> Result<PathBuf, RuntimeError> {
        Self::ensure_not_cancelled(cancel_requested)?;
        let executable = installation_root.join("soh.exe");
        let resource_archive = installation_root.join("soh.o2r");
        let marker = installation_root.join(".sre-shipwright-runtime-version");
        let version = format!("{}\n", bundle.metadata.shipwright_version);
        let runtime_ready = fs::read_to_string(&marker).ok().as_deref() == Some(&version)
            && Self::files_match(&bundle.executable, &executable)
            && Self::files_match(&bundle.resource_archive, &resource_archive);

        if !runtime_ready {
            Self::copy_file_atomically_cancellable(
                &bundle.executable,
                &executable,
                cancel_requested,
            )?;
            Self::copy_file_atomically_cancellable(
                &bundle.resource_archive,
                &resource_archive,
                cancel_requested,
            )?;
        }
        self.stage_runtime_assets_cancellable(bundle, installation_root, cancel_requested)?;
        if !runtime_ready {
            Self::ensure_not_cancelled(cancel_requested)?;
            Self::atomic_write(&marker, version.as_bytes())?;
        }
        Ok(executable)
    }

    fn first_existing_file(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
        candidates.into_iter().find(|path| path.is_file())
    }

    fn first_existing_directory(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
        candidates.into_iter().find(|path| path.is_dir())
    }

    fn extractor(&self) -> Option<PathBuf> {
        Self::first_existing_file(self.integration_roots.iter().flat_map(|root| {
            [
                root.join("extractor/soh-torch.exe"),
                root.join("resources/extractor/soh-torch.exe"),
                root.join("build-ftep-tools/soh-torch.exe"),
                root.join("build-ftep-tools/Release/soh-torch.exe"),
                root.join("apps/launcher/resources/extractor/soh-torch.exe"),
            ]
        }))
    }

    fn extractor_definitions(&self) -> Option<PathBuf> {
        Self::first_existing_directory(self.integration_roots.iter().flat_map(|root| {
            [
                root.join("extractor/yml"),
                root.join("resources/extractor/yml"),
                root.join("soh/assets/yml"),
                root.join("apps/launcher/resources/extractor/yml"),
            ]
        }))
    }

    fn extractor_assets(&self) -> Option<PathBuf> {
        Self::first_existing_directory(self.integration_roots.iter().flat_map(|root| {
            [
                root.join("extractor/assets"),
                root.join("resources/extractor/assets"),
                root.join("apps/launcher/resources/extractor/assets"),
            ]
        }))
    }

    fn extractor_working_directory(extractor: &Path) -> Result<PathBuf, RuntimeError> {
        extractor.parent().map(Path::to_path_buf).ok_or_else(|| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Shipwright extractor has no containing directory.",
                extractor.display().to_string(),
            )
        })
    }

    pub fn pipeline_available(&self) -> bool {
        self.extractor().is_some()
            && self.extractor_definitions().is_some()
            && self.extractor_assets().is_some()
    }

    fn error(
        code: RuntimeErrorCode,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> RuntimeError {
        RuntimeError::new(code, message).with_technical_details(details)
    }

    fn hex(bytes: &[u8]) -> String {
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

    fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
        let parent = path.parent().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Imported asset metadata has no parent directory.",
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Imported asset metadata directory could not be prepared.",
                error.to_string(),
            )
        })?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".sre-import-")
            .tempfile_in(parent)
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Imported asset metadata staging could not be created.",
                    error.to_string(),
                )
            })?;
        temporary
            .write_all(bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::Internal,
                    "Imported asset metadata could not be written safely.",
                    error.to_string(),
                )
            })?;
        temporary.persist(path).map_err(|error| {
            Self::error(
                RuntimeErrorCode::Internal,
                "Imported asset metadata could not be promoted safely.",
                error.error.to_string(),
            )
        })?;
        Ok(())
    }

    pub fn validate_oot_source(&self, path: &Path) -> Result<OotSourceValidation, RuntimeError> {
        let file = fs::File::open(path).map_err(|error| {
            Self::error(
                RuntimeErrorCode::GameSourceMissing,
                "Unable to read the selected game data.",
                error.to_string(),
            )
        })?;
        let metadata = file.metadata().map_err(|error| {
            Self::error(
                RuntimeErrorCode::GameSourceMissing,
                "Unable to inspect the selected game data.",
                error.to_string(),
            )
        })?;
        let mut reader = BufReader::with_capacity(1024 * 1024, file);
        let mut header = [0_u8; 4];
        reader.read_exact(&mut header).map_err(|error| {
            Self::error(
                RuntimeErrorCode::GameSourceInvalid,
                "The selected game data is too small to be recognized.",
                error.to_string(),
            )
        })?;
        let mut hasher = Sha1::new();
        hasher.update(header);
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = reader.read(&mut buffer).map_err(|error| {
                Self::error(
                    RuntimeErrorCode::GameSourceInvalid,
                    "Reading the selected game data failed.",
                    error.to_string(),
                )
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let sha1 = Self::hex(&hasher.finalize());
        let hashes: Vec<SupportedHash> =
            serde_json::from_str(SUPPORTED_HASHES_JSON).map_err(|error| {
                Self::error(
                    RuntimeErrorCode::Internal,
                    "The Shipwright supported game-data catalog is invalid.",
                    error.to_string(),
                )
            })?;
        let supported = hashes
            .iter()
            .find(|entry| entry.sha1.eq_ignore_ascii_case(&sha1));
        let format_name = Self::rom_format(header).map(str::to_owned);
        Ok(OotSourceValidation {
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
            import_pipeline_available: self.pipeline_available(),
            sha1,
        })
    }

    pub fn current_oot_asset_directory(
        &self,
        assets_root: &Path,
    ) -> Result<Option<PathBuf>, RuntimeError> {
        let manifest_path = assets_root.join("current.json");
        if !manifest_path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(&manifest_path).map_err(|error| {
            Self::error(
                RuntimeErrorCode::GameSourceInvalid,
                "Imported asset metadata could not be read.",
                error.to_string(),
            )
        })?;
        let manifest: OotAssetManifest = serde_json::from_slice(&bytes).map_err(|error| {
            Self::error(
                RuntimeErrorCode::GameSourceInvalid,
                "Imported asset metadata is invalid.",
                error.to_string(),
            )
        })?;
        let directory = assets_root.join("installs").join(manifest.installation_id);
        Ok(directory
            .join(manifest.archive_name)
            .is_file()
            .then_some(directory))
    }

    pub fn import_oot_assets(
        &self,
        request: &OotImportRequest,
        assets_root: &Path,
        cancel_requested: &AtomicBool,
    ) -> Result<OotImportReport, RuntimeError> {
        let report = self.validate_oot_source(&request.path)?;
        if !report.version_supported
            || !report.integrity_validated
            || !report.import_pipeline_available
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "The supplied game data is not supported by the Shipwright import pipeline.",
            ));
        }
        if !report.sha1.eq_ignore_ascii_case(&request.expected_sha1) {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "The selected game data changed after validation. Please validate it again.",
            ));
        }
        let extractor = self.extractor().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeNotInstalled,
                "The maintained Shipwright extractor is not installed.",
            )
        })?;
        let definitions = self.extractor_definitions().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeNotInstalled,
                "Shipwright extractor definitions are not installed.",
            )
        })?;
        let extractor_working_directory = Self::extractor_working_directory(&extractor)?;
        fs::create_dir_all(assets_root.join("staging"))
            .and_then(|_| fs::create_dir_all(assets_root.join("installs")))
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Asset directories could not be prepared.",
                    error.to_string(),
                )
            })?;
        let required_space = MINIMUM_IMPORT_SPACE.max(report.file_size.saturating_mul(8));
        let available_space = fs2::available_space(assets_root).map_err(|error| {
            Self::error(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "Free disk space could not be measured.",
                error.to_string(),
            )
        })?;
        if available_space < required_space {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                format!(
                    "Asset import requires at least {:.1} GiB free.",
                    required_space as f64 / 1024_f64.powi(3)
                ),
            ));
        }
        let staging = tempfile::Builder::new()
            .prefix("import-")
            .tempdir_in(assets_root.join("staging"))
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeConfigurationInvalid,
                    "Import staging could not be created.",
                    error.to_string(),
                )
            })?;
        let stdout_path = staging.path().join("extractor.stdout.log");
        let stderr_path = staging.path().join("extractor.stderr.log");
        let stdout = fs::File::create(&stdout_path).map_err(|error| {
            Self::error(
                RuntimeErrorCode::Internal,
                "Import diagnostics could not be prepared.",
                error.to_string(),
            )
        })?;
        let stderr = fs::File::create(&stderr_path).map_err(|error| {
            Self::error(
                RuntimeErrorCode::Internal,
                "Import diagnostics could not be prepared.",
                error.to_string(),
            )
        })?;
        let mut child = Command::new(extractor)
            .current_dir(extractor_working_directory)
            .arg("--src")
            .arg(definitions)
            .arg("--dest")
            .arg(staging.path())
            .arg("--version")
            .arg(SHIPWRIGHT_VERSION)
            .arg(&request.path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::RuntimeLaunchFailed,
                    "Shipwright extraction could not start.",
                    error.to_string(),
                )
            })?;
        let exit_status = loop {
            if cancel_requested.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::new(
                    RuntimeErrorCode::GameSourceInvalid,
                    "Asset import was cancelled. The original file was not changed.",
                ));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(100)),
                Err(error) => {
                    let _ = child.kill();
                    return Err(Self::error(
                        RuntimeErrorCode::Internal,
                        "Asset import could not be monitored.",
                        error.to_string(),
                    ));
                }
            }
        };
        if !exit_status.success() {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                format!(
                    "Shipwright extraction failed with exit code {}. Technical logs were discarded with staging data.",
                    exit_status.code().unwrap_or(-1)
                ),
            ));
        }
        let mut archives = fs::read_dir(staging.path())
            .map_err(|error| {
                Self::error(
                    RuntimeErrorCode::Internal,
                    "Import output could not be inspected.",
                    error.to_string(),
                )
            })?
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
            RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "Shipwright produced no usable game-data archive.",
            )
        })?;
        if archive
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0)
            == 0
        {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameSourceInvalid,
                "Shipwright produced an empty game-data archive.",
            ));
        }
        let _ = fs::remove_file(staging.path().join("torch.hash.yml"));
        let _ = fs::remove_file(stdout_path);
        let _ = fs::remove_file(stderr_path);
        let imported_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::Internal,
                    "System clock is before the Unix epoch.",
                )
            })?
            .as_millis();
        let installation_id = format!("{}-{}", imported_at_unix_ms, std::process::id());
        let installation_dir = assets_root.join("installs").join(&installation_id);
        let archive_name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("oot.o2r")
            .to_owned();
        fs::rename(staging.path(), &installation_dir).map_err(|error| {
            Self::error(
                RuntimeErrorCode::Internal,
                "Completed assets could not be promoted.",
                error.to_string(),
            )
        })?;
        let manifest = OotAssetManifest {
            schema_version: 1,
            installation_id: installation_id.clone(),
            archive_name: archive_name.clone(),
            detected_version: request.detected_version.clone(),
            source_sha1: report.sha1,
            shipwright_version: SHIPWRIGHT_VERSION.to_owned(),
            completed_at_unix_ms: imported_at_unix_ms,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
            Self::error(
                RuntimeErrorCode::Internal,
                "Import metadata could not be serialized.",
                error.to_string(),
            )
        })?;
        if let Err(error) = Self::atomic_write(&assets_root.join("current.json"), &manifest_bytes) {
            let _ = fs::remove_dir_all(&installation_dir);
            return Err(error);
        }
        Ok(OotImportReport {
            archive_name,
            installation_id,
            imported_at_unix_ms,
        })
    }
}

impl ShipwrightAdapter {
    /// Starts Shipwright while allowing the caller to cancel the expensive
    /// first-run runtime staging before any game process is created.
    pub fn launch_cancellable(
        &self,
        request: LaunchRequest,
        cancel_requested: &AtomicBool,
    ) -> Result<RuntimeSession, RuntimeError> {
        Self::ensure_not_cancelled(cancel_requested)?;
        if request.mode != LaunchMode::Authorized || request.installation.synthetic_fixture {
            return Err(RuntimeError::new(
                RuntimeErrorCode::SyntheticLaunchForbidden,
                "Shipwright refuses synthetic fixture launches.",
            ));
        }
        if request.session_id.trim().is_empty() {
            return Err(RuntimeError::new(
                RuntimeErrorCode::RuntimeConfigurationInvalid,
                "A non-empty FTEP session id is required.",
            ));
        }
        self.configure(&request.installation, &request.config)?;
        Self::ensure_not_cancelled(cancel_requested)?;

        let bundle = self.find_bundle()?.ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::RuntimeNotInstalled,
                "The verified Shipwright runtime is not installed.",
            )
        })?;
        let executable = self.stage_runtime_bundle_cancellable(
            &bundle,
            &request.installation.root,
            cancel_requested,
        )?;
        Self::ensure_not_cancelled(cancel_requested)?;

        let child = Command::new(executable)
            .current_dir(&request.installation.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeLaunchFailed,
                    "Shipwright could not be started.",
                )
                .with_technical_details(error.to_string())
            })?;

        let process_id = child.id();
        self.children
            .lock()
            .map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::Internal,
                    "Runtime process registry is unavailable.",
                )
            })?
            .insert(process_id, child);

        Ok(RuntimeSession {
            session_id: request.session_id,
            game_id: request.installation.game_id,
            variant_id: request.installation.variant_id,
            runtime_id: self.id.clone(),
            device_id: request.config.values.get("device_id").cloned(),
            state: RuntimeSessionState::RuntimeStarted,
            process_id: Some(process_id),
            synthetic_fixture: false,
            requested_at_unix_ms: now_unix_ms(),
            started_at_unix_ms: Some(now_unix_ms()),
            playable_at_unix_ms: None,
            ended_at_unix_ms: None,
            duration_ms: None,
            exit_code: None,
            launch_result: "PROCESS_STARTED".to_owned(),
            initial_events: vec![],
        })
    }
}

impl RuntimeProvider for ShipwrightAdapter {
    fn id(&self) -> &RuntimeId {
        &self.id
    }

    fn display_name(&self) -> &str {
        "Ship of Harkinian"
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::NativePort
    }

    fn distribution_mode(&self) -> DistributionMode {
        DistributionMode::Bundled
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            native: true,
            external_process: true,
            supports_overlay: false,
            supports_semantic_events: false,
            supports_save_detection: false,
            supports_controller_config: false,
            supports_mods: false,
        }
    }

    fn detect(&self) -> Result<DetectionResult, RuntimeError> {
        let Some(bundle) = self.find_bundle()? else {
            return Ok(DetectionResult {
                runtime_id: self.id.clone(),
                status: DetectionStatus::Missing,
                installation: None,
                summary: "The verified Shipwright runtime is not installed.".to_owned(),
            });
        };

        Ok(DetectionResult {
            runtime_id: self.id.clone(),
            status: DetectionStatus::Available,
            installation: Some(RuntimeInstallation {
                runtime_id: self.id.clone(),
                root: bundle.root,
                executable: Some(bundle.executable),
                version: Some(bundle.metadata.shipwright_version),
            }),
            summary: "Shipwright runtime located and protocol-compatible.".to_owned(),
        })
    }

    fn validate_version(
        &self,
        installation: &RuntimeInstallation,
    ) -> Result<VersionValidation, RuntimeError> {
        let current = self.find_bundle()?;
        let supported = current.as_ref().is_some_and(|bundle| {
            bundle.root == installation.root
                && installation.version.as_deref() == Some(&bundle.metadata.shipwright_version)
        });
        Ok(VersionValidation {
            status: if supported {
                VersionStatus::Supported
            } else {
                VersionStatus::Unsupported
            },
            detected_version: installation.version.clone(),
            summary: if supported {
                "Shipwright runtime protocol and platform are supported."
            } else {
                "Shipwright runtime metadata does not match the detected bundle."
            }
            .to_owned(),
        })
    }

    fn validate_source(
        &self,
        game: &GameDefinition,
        source: &GameSource,
    ) -> Result<SourceValidation, RuntimeError> {
        let valid_game = game.id.as_str() == "zelda-oot" && source.variant_id.as_str() == "n64";
        if !valid_game || source.synthetic_fixture {
            return Ok(SourceValidation {
                valid: false,
                detected_variant: None,
                summary: "Shipwright accepts only real Ocarina of Time Nintendo 64 game data."
                    .to_owned(),
            });
        }
        let report = self.validate_oot_source(&source.path)?;
        Ok(SourceValidation {
            valid: report.version_supported && report.integrity_validated,
            detected_variant: report.version_supported.then(|| source.variant_id.clone()),
            summary: if report.version_supported {
                "Supported OoT game data recognized."
            } else {
                "The supplied OoT game data is not supported by Shipwright."
            }
            .to_owned(),
        })
    }

    fn prepare(
        &self,
        _game: &GameDefinition,
        _source: &GameSource,
        _context: &PreparationContext,
    ) -> Result<PreparedGame, RuntimeError> {
        Err(RuntimeError::new(
            RuntimeErrorCode::OperationUnsupported,
            "Use the cancellable Shipwright import API for desktop asset preparation.",
        ))
    }

    fn verify(&self, installation: &GameInstallation) -> Result<VerificationResult, RuntimeError> {
        let provider_matches = installation.runtime_id == self.id;
        let archive_present = installation.root.join("oot.o2r").is_file()
            || installation.root.join("oot-mq.o2r").is_file();
        let ready = provider_matches && !installation.synthetic_fixture && archive_present;
        Ok(VerificationResult {
            ready,
            summary: if ready {
                "Imported OoT assets are ready for Shipwright."
            } else {
                "The Shipwright installation is missing a verified imported OoT archive."
            }
            .to_owned(),
        })
    }

    fn configure(
        &self,
        installation: &GameInstallation,
        _config: &RuntimeConfig,
    ) -> Result<(), RuntimeError> {
        if !self.verify(installation)?.ready {
            return Err(RuntimeError::new(
                RuntimeErrorCode::GameNotConfigured,
                "Shipwright game assets are not ready.",
            ));
        }
        Ok(())
    }

    fn launch(&self, request: LaunchRequest) -> Result<RuntimeSession, RuntimeError> {
        self.launch_cancellable(request, &AtomicBool::new(false))
    }

    fn observe_process(
        &self,
        session: &RuntimeSession,
    ) -> Result<ProcessObservation, RuntimeError> {
        let Some(process_id) = session.process_id else {
            return Ok(ProcessObservation {
                running: false,
                observed_at_unix_ms: now_unix_ms(),
                exit_code: session.exit_code,
            });
        };
        let mut children = self.children.lock().map_err(|_| {
            RuntimeError::new(
                RuntimeErrorCode::Internal,
                "Runtime process registry is unavailable.",
            )
        })?;
        let Some(child) = children.get_mut(&process_id) else {
            return Ok(ProcessObservation {
                running: session.exit_code.is_none(),
                observed_at_unix_ms: now_unix_ms(),
                exit_code: session.exit_code,
            });
        };
        let exit = child.try_wait().map_err(|error| {
            RuntimeError::new(
                RuntimeErrorCode::Internal,
                "Shipwright process state could not be observed.",
            )
            .with_technical_details(error.to_string())
        })?;
        Ok(ProcessObservation {
            running: exit.is_none(),
            observed_at_unix_ms: now_unix_ms(),
            exit_code: exit.and_then(|status| status.code()),
        })
    }

    fn determine_playable_state(
        &self,
        session: &RuntimeSession,
    ) -> Result<PlayableDetection, RuntimeError> {
        let observation = self.observe_process(session)?;
        let elapsed = session
            .started_at_unix_ms
            .map(|started| observation.observed_at_unix_ms.saturating_sub(started))
            .unwrap_or(0);
        Ok(PlayableDetection {
            playable: observation.running && elapsed >= 3_000,
            precision: PlayablePrecision::Approximate,
            method: "process alive plus 3 second startup threshold".to_owned(),
            observed_at_unix_ms: observation.observed_at_unix_ms,
        })
    }

    fn find_save_location(
        &self,
        installation: &GameInstallation,
    ) -> Result<Option<SaveLocation>, RuntimeError> {
        let path = installation.root.join("Save");
        Ok(path.exists().then_some(SaveLocation {
            path,
            confidence: PlayablePrecision::Approximate,
            summary: "Shipwright save directory detected beside the imported assets.".to_owned(),
        }))
    }

    fn diagnostics(
        &self,
        installation: Option<&GameInstallation>,
    ) -> Result<Vec<RuntimeDiagnostic>, RuntimeError> {
        let detection = self.detect()?;
        let mut findings = vec![RuntimeDiagnostic {
            id: "shipwright-runtime".to_owned(),
            severity: if detection.status == DetectionStatus::Available {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            summary: detection.summary,
            remediation: (detection.status != DetectionStatus::Available)
                .then(|| "Repair or reinstall the SRE-bundled native runtime.".to_owned()),
        }];
        if let Some(installation) = installation {
            let verification = self.verify(installation)?;
            findings.push(RuntimeDiagnostic {
                id: "shipwright-game".to_owned(),
                severity: if verification.ready {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Error
                },
                summary: verification.summary,
                remediation: (!verification.ready)
                    .then(|| "Re-import legally supplied compatible game data.".to_owned()),
            });
        }
        Ok(findings)
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sre_core::{GameId, GameVariantId};
    use sre_runtime::{LaunchMode, RuntimeConfig};

    fn write_bundle(root: &Path, protocol: u32) {
        fs::write(root.join("soh.exe"), b"exe").unwrap();
        fs::write(root.join("soh.o2r"), b"resource").unwrap();
        fs::write(
            root.join("sre-runtime.json"),
            format!(
                r#"{{"schema_version":1,"shipwright_version":"9.2.3","runtime_protocol":{protocol},"platform":"windows-x64"}}"#
            ),
        )
        .unwrap();
    }

    fn write_extractor_assets(root: &Path) {
        let nested = root.join("extractor/assets/nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("fixture.bin"), b"assets").unwrap();
    }

    fn fixture_installation(root: PathBuf) -> GameInstallation {
        GameInstallation {
            installation_id: "fixture-installation".to_owned(),
            game_id: GameId::new("zelda-oot").unwrap(),
            variant_id: GameVariantId::new("n64").unwrap(),
            runtime_id: RuntimeId::new("shipwright").unwrap(),
            root,
            synthetic_fixture: false,
        }
    }

    #[test]
    fn detects_a_protocol_compatible_runtime() {
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), SHIPWRIGHT_RUNTIME_PROTOCOL);
        let adapter = ShipwrightAdapter::new([directory.path().to_owned()]);
        let detection = adapter.detect().unwrap();
        assert_eq!(detection.status, DetectionStatus::Available);
        assert_eq!(
            detection.installation.unwrap().version.as_deref(),
            Some("9.2.3")
        );
    }

    #[test]
    fn rejects_incompatible_runtime_metadata() {
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), SHIPWRIGHT_RUNTIME_PROTOCOL + 1);
        let adapter = ShipwrightAdapter::new([directory.path().to_owned()]);
        assert_eq!(
            adapter.detect().unwrap_err().code,
            RuntimeErrorCode::RuntimeVersionUnsupported
        );
    }

    #[test]
    fn verification_requires_real_imported_oot_assets() {
        let directory = tempfile::tempdir().unwrap();
        let adapter = ShipwrightAdapter::new([]);
        let installation = fixture_installation(directory.path().to_owned());
        assert!(!adapter.verify(&installation).unwrap().ready);
        fs::write(directory.path().join("oot.o2r"), b"assets").unwrap();
        assert!(adapter.verify(&installation).unwrap().ready);
    }

    #[test]
    fn extractor_runs_from_its_containing_resource_directory() {
        assert_eq!(
            ShipwrightAdapter::extractor_working_directory(Path::new(
                "resources/extractor/soh-torch.exe"
            ))
            .unwrap(),
            PathBuf::from("resources/extractor")
        );
    }

    #[test]
    fn recognizes_all_nintendo_64_byte_orders() {
        assert!(ShipwrightAdapter::rom_format([0x80, 0x37, 0x12, 0x40]).is_some());
        assert!(ShipwrightAdapter::rom_format([0x37, 0x80, 0x40, 0x12]).is_some());
        assert!(ShipwrightAdapter::rom_format([0x40, 0x12, 0x37, 0x80]).is_some());
        assert!(ShipwrightAdapter::rom_format([0, 0, 0, 0]).is_none());
    }

    #[test]
    fn bundled_source_hash_catalog_is_well_formed() {
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
    fn atomic_resource_copy_preserves_content() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.o2r");
        let destination = directory.path().join("destination.o2r");
        fs::write(&destination, b"same-len").unwrap();
        fs::write(&source, b"resource").unwrap();
        ShipwrightAdapter::copy_file_atomically_cancellable(
            &source,
            &destination,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"resource");
    }

    #[test]
    fn runtime_bundle_is_staged_beside_imported_game_archives() {
        let runtime = tempfile::tempdir().unwrap();
        let integration = tempfile::tempdir().unwrap();
        write_bundle(runtime.path(), SHIPWRIGHT_RUNTIME_PROTOCOL);
        write_extractor_assets(integration.path());
        let adapter = ShipwrightAdapter::new([runtime.path().to_owned()])
            .with_integration_roots([integration.path().to_owned()]);
        let bundle = adapter.find_bundle().unwrap().unwrap();
        let installation = tempfile::tempdir().unwrap();

        let executable = adapter
            .stage_runtime_bundle_cancellable(&bundle, installation.path(), &AtomicBool::new(false))
            .unwrap();

        assert_eq!(executable, installation.path().join("soh.exe"));
        assert_eq!(fs::read(executable).unwrap(), b"exe");
        assert_eq!(
            fs::read(installation.path().join("soh.o2r")).unwrap(),
            b"resource"
        );
        assert_eq!(
            fs::read(installation.path().join("assets/nested/fixture.bin")).unwrap(),
            b"assets"
        );
        assert_eq!(
            fs::read_to_string(installation.path().join(".sre-shipwright-assets-version")).unwrap(),
            "9.2.3\n"
        );
        assert_eq!(
            fs::read_to_string(installation.path().join(".sre-shipwright-runtime-version"))
                .unwrap(),
            "9.2.3\n"
        );
    }

    #[test]
    fn matching_runtime_marker_reuses_existing_runtime_files() {
        let runtime = tempfile::tempdir().unwrap();
        let integration = tempfile::tempdir().unwrap();
        write_bundle(runtime.path(), SHIPWRIGHT_RUNTIME_PROTOCOL);
        write_extractor_assets(integration.path());
        let adapter = ShipwrightAdapter::new([runtime.path().to_owned()])
            .with_integration_roots([integration.path().to_owned()]);
        let bundle = adapter.find_bundle().unwrap().unwrap();
        let installation = tempfile::tempdir().unwrap();

        adapter
            .stage_runtime_bundle_cancellable(&bundle, installation.path(), &AtomicBool::new(false))
            .unwrap();
        fs::write(&bundle.executable, b"new").unwrap();
        adapter
            .stage_runtime_bundle_cancellable(&bundle, installation.path(), &AtomicBool::new(false))
            .unwrap();

        assert_eq!(
            fs::read(installation.path().join("soh.exe")).unwrap(),
            b"exe"
        );
    }

    #[test]
    fn runtime_staging_honors_a_cancel_request() {
        let runtime = tempfile::tempdir().unwrap();
        let integration = tempfile::tempdir().unwrap();
        write_bundle(runtime.path(), SHIPWRIGHT_RUNTIME_PROTOCOL);
        write_extractor_assets(integration.path());
        let adapter = ShipwrightAdapter::new([runtime.path().to_owned()])
            .with_integration_roots([integration.path().to_owned()]);
        let bundle = adapter.find_bundle().unwrap().unwrap();
        let installation = tempfile::tempdir().unwrap();
        let cancelled = AtomicBool::new(true);

        let error = adapter
            .stage_runtime_bundle_cancellable(&bundle, installation.path(), &cancelled)
            .unwrap_err();

        assert_eq!(error.code, RuntimeErrorCode::OperationCancelled);
        assert!(!installation.path().join("soh.exe").exists());
    }

    #[test]
    fn atomic_manifest_write_replaces_existing_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("current.json");
        ShipwrightAdapter::atomic_write(&path, b"first").unwrap();
        ShipwrightAdapter::atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"second");
    }

    #[test]
    fn refuses_synthetic_launch_before_process_creation() {
        let adapter = ShipwrightAdapter::new([]);
        let mut installation = fixture_installation(PathBuf::from("fixture"));
        installation.synthetic_fixture = true;
        let error = adapter
            .launch(LaunchRequest {
                session_id: "session-1".to_owned(),
                installation,
                config: RuntimeConfig::default(),
                mode: LaunchMode::SyntheticFixture,
            })
            .unwrap_err();
        assert_eq!(error.code, RuntimeErrorCode::SyntheticLaunchForbidden);
    }
}
