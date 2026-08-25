use crate::ftep_client;
use crate::storage;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use sre_achievements::{AchievementEngine, AchievementEvent};
use sre_device::DeviceStore;
use sre_diagnostics::{DoctorReport, export_sanitized, run_doctor};
use sre_dolphin_adapter::DolphinRuntimeAdapter;
use sre_entitlement::{EntitlementLease, LeaseStatus, LeaseVerifier, SignedLease, allows_game};
use sre_library::{LibraryStore, SessionHistory, SessionStore};
use sre_overlay::{OverlayNotification, OverlayQueue};
use sre_runtime::{
    GameInstallation, LaunchMode, LaunchRequest, PlayablePrecision, RuntimeConfig, RuntimeEvent,
    RuntimeProvider, RuntimeSession, RuntimeSessionState,
};
use sre_switch_adapter::{ExternalSwitchImplementation, SwitchRuntimeProvider};
use sre_wiiu_adapter::WiiURuntimeAdapter;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tauri::Manager;
use url::Url;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
    },
    core::PCWSTR,
};

#[cfg(windows)]
const LOCAL_FTEP_PACKAGE_MANAGER: &str = "pnpm.cmd";
#[cfg(not(windows))]
const LOCAL_FTEP_PACKAGE_MANAGER: &str = "pnpm";
const LOCAL_FTEP_DATABASE_CONTAINER: &str = "ftep-local-postgres";
const DOCKER_DAEMON_TIMEOUT: Duration = Duration::from_secs(60);
const DOCKER_POLL_INTERVAL: Duration = Duration::from_millis(500);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Default)]
struct ManagedLocalDatabase {
    container_started_by_sre: bool,
    desktop_started_by_sre: bool,
}

impl ManagedLocalDatabase {
    fn shutdown(self) {
        if self.container_started_by_sre {
            let _ = run_docker(&["stop", "--timeout", "10", LOCAL_FTEP_DATABASE_CONTAINER]);
        }

        // Docker Desktop itself is shared infrastructure. Only stop it when
        // SRE had to start it and no container is still running at shutdown;
        // this leaves unrelated user workloads alone.
        if self.desktop_started_by_sre && matches!(running_docker_containers(), Ok(false)) {
            let _ = run_docker(&["desktop", "stop", "--detach"]);
        }
    }
}

#[derive(Debug)]
struct ManagedLocalFtep {
    child: Child,
    web_url: String,
    database: ManagedLocalDatabase,
    // A Windows job object owns pnpm and every descendant it starts. If SRE is
    // force-closed, Windows closes this handle and ends that complete local
    // service tree instead of leaving Next.js and its port behind.
    #[cfg(windows)]
    _job: LocalFtepJob,
}

impl ManagedLocalFtep {
    fn shutdown(mut self) {
        stop_local_ftep_process(&mut self.child);
        self.database.shutdown();
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct LocalFtepJob(isize);

#[cfg(windows)]
impl Drop for LocalFtepJob {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE is configured before the handle is retained.
        unsafe {
            let _ = CloseHandle(HANDLE(self.0 as *mut _));
        }
    }
}

#[derive(Clone)]
pub(crate) struct LocalServices {
    pub(crate) overlay: OverlayQueue,
    session_write: Arc<Mutex<()>>,
    managed_local_ftep: Arc<Mutex<Option<ManagedLocalFtep>>>,
    active_launch_cancellation: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    active_game_processes: Arc<Mutex<BTreeMap<String, u32>>>,
}

impl Default for LocalServices {
    fn default() -> Self {
        Self {
            overlay: OverlayQueue::default(),
            session_write: Arc::new(Mutex::new(())),
            managed_local_ftep: Arc::new(Mutex::new(None)),
            active_launch_cancellation: Arc::new(Mutex::new(None)),
            active_game_processes: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl LocalServices {
    fn begin_game_launch(&self) -> Result<Arc<AtomicBool>, String> {
        let mut active = self.active_launch_cancellation.lock().map_err(|_| {
            "FICSIT-0001: Game launch cancellation state is unavailable.".to_owned()
        })?;
        if active.is_some() {
            return Err("FICSIT-0001: Another game is already being prepared.".to_owned());
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        *active = Some(cancellation.clone());
        Ok(cancellation)
    }

    fn finish_game_launch(&self, cancellation: &Arc<AtomicBool>) {
        if let Ok(mut active) = self.active_launch_cancellation.lock()
            && active
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, cancellation))
        {
            *active = None;
        }
    }

    fn cancel_game_launch(&self) -> Result<(), String> {
        let active = self.active_launch_cancellation.lock().map_err(|_| {
            "FICSIT-0001: Game launch cancellation state is unavailable.".to_owned()
        })?;
        let cancellation = active
            .as_ref()
            .ok_or_else(|| "FICSIT-0001: No game launch is currently being prepared.".to_owned())?;
        cancellation.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn track_game_process(&self, session_id: &str, process_id: u32) {
        if let Ok(mut active) = self.active_game_processes.lock() {
            active.insert(session_id.to_owned(), process_id);
        }
    }

    fn release_game_process(&self, session_id: &str) {
        if let Ok(mut active) = self.active_game_processes.lock() {
            active.remove(session_id);
        }
    }

    fn active_game_session_ids(&self) -> Result<Vec<String>, String> {
        Ok(self
            .active_game_processes
            .lock()
            .map_err(|_| "FICSIT-0001: Active game process state is unavailable.".to_owned())?
            .keys()
            .cloned()
            .collect())
    }

    fn stop_game_process(&self, session_id: &str) -> Result<(), String> {
        let process_id = self
            .active_game_processes
            .lock()
            .map_err(|_| "FICSIT-0001: Active game process state is unavailable.".to_owned())?
            .get(session_id)
            .copied()
            .ok_or_else(|| {
                "FICSIT-0001: This launcher is not currently managing that game process.".to_owned()
            })?;
        terminate_owned_game_process(process_id)?;
        self.release_game_process(session_id);
        Ok(())
    }

    pub(crate) fn start_managed_local_ftep(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if !cfg!(debug_assertions) {
            return Ok(());
        }

        let mut managed = self
            .managed_local_ftep
            .lock()
            .map_err(|_| "FICSIT-0002: Local FTEP service state is unavailable.".to_owned())?;
        if let Some(existing) = managed.as_mut() {
            if existing
                .child
                .try_wait()
                .map_err(|error| format!("FICSIT-0002: Cannot inspect local FTEP: {error}"))?
                .is_none()
            {
                return Ok(());
            }
        }
        if let Some(existing) = managed.take() {
            existing.shutdown();
        }

        let port = reserve_loopback_port()?;
        let web_url = format!("http://127.0.0.1:{port}");
        let workspace = local_workspace_root()?;
        let log_path = storage::app_data_dir(app)?.join("local-ftep.log");
        let mut log_file = File::create(&log_path)
            .map_err(|error| format!("FICSIT-0002: Cannot create local FTEP log file: {error}"))?;
        let database = start_managed_local_database(&mut log_file);
        let stdout = match log_file.try_clone() {
            Ok(stdout) => stdout,
            Err(error) => {
                database.shutdown();
                return Err(format!(
                    "FICSIT-0002: Cannot prepare local FTEP log file: {error}"
                ));
            }
        };
        let port_text = port.to_string();
        let mut child = match Command::new(LOCAL_FTEP_PACKAGE_MANAGER)
            .args([
                "--filter",
                "@ftep/web",
                "dev",
                "--hostname",
                "127.0.0.1",
                "--port",
                &port_text,
            ])
            .current_dir(workspace)
            .env("NEXT_PUBLIC_FTEP_BASE_URL", &web_url)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(log_file))
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                database.shutdown();
                return Err(format!(
                    "FICSIT-0002: Cannot start local FTEP. Install pnpm and see {}: {error}",
                    log_path.display()
                ));
            }
        };

        #[cfg(windows)]
        let job = match contain_local_ftep_process(&child) {
            Ok(job) => job,
            Err(error) => {
                stop_local_ftep_process(&mut child);
                database.shutdown();
                return Err(error);
            }
        };

        // Do not wait for Next.js here. This method runs from Tauri's setup
        // hook, and waiting for the local FTEP health check would leave the
        // native window white and unresponsive for up to 45 seconds. The
        // managed child is retained and health is checked when a command uses
        // the service; the launcher UI can render immediately.
        *managed = Some(ManagedLocalFtep {
            child,
            web_url,
            database,
            #[cfg(windows)]
            _job: job,
        });
        Ok(())
    }

    fn managed_local_ftep_url(&self) -> Result<String, String> {
        if !cfg!(debug_assertions) {
            let configured = std::env::var("FTEP_WEB_URL")
                .ok()
                .or_else(|| option_env!("VITE_FTEP_WEB_URL").map(str::to_owned))
                .ok_or_else(|| {
                    "FICSIT-0002: This release build requires a configured HTTPS FTEP service."
                        .to_owned()
                })?;
            let url = Url::parse(&configured)
                .map_err(|_| "FICSIT-0002: The configured FTEP web URL is invalid.".to_owned())?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(
                    "FICSIT-0002: This release build requires a configured HTTPS FTEP service."
                        .to_owned(),
                );
            }
            return Ok(configured.trim_end_matches('/').to_owned());
        }

        let mut managed = self
            .managed_local_ftep
            .lock()
            .map_err(|_| "FICSIT-0002: Local FTEP service state is unavailable.".to_owned())?;
        let Some(local_ftep) = managed.as_mut() else {
            return Err(
                "FICSIT-0002: Local FTEP is not running. Restart SRE to start it automatically."
                    .to_owned(),
            );
        };
        let finished = local_ftep
            .child
            .try_wait()
            .map_err(|error| format!("FICSIT-0002: Cannot inspect local FTEP: {error}"))?
            .is_some();
        if finished {
            if let Some(local_ftep) = managed.take() {
                local_ftep.shutdown();
            }
            return Err(
                "FICSIT-0002: Local FTEP stopped unexpectedly. Restart SRE to start it again."
                    .to_owned(),
            );
        }
        Ok(local_ftep.web_url.clone())
    }

    pub(crate) fn shutdown_managed_local_ftep(&self) {
        let local_ftep = self
            .managed_local_ftep
            .lock()
            .ok()
            .and_then(|mut managed| managed.take());
        if let Some(local_ftep) = local_ftep {
            local_ftep.shutdown();
        }
    }
}

fn start_managed_local_database(log: &mut File) -> ManagedLocalDatabase {
    let mut managed = ManagedLocalDatabase::default();
    let daemon_ready = match docker_daemon_is_ready() {
        Ok(ready) => ready,
        Err(error) => {
            let _ = writeln!(
                log,
                "Docker is unavailable; using file-backed FTEP state: {error}"
            );
            return managed;
        }
    };

    if !daemon_ready {
        match run_docker(&["desktop", "start", "--detach"]) {
            Ok(output) if output.status.success() => {
                managed.desktop_started_by_sre = true;
                let _ = writeln!(log, "Started Docker Desktop for local FTEP.");
            }
            Ok(output) => {
                let _ = writeln!(
                    log,
                    "Docker Desktop could not be started; using file-backed FTEP state: {}",
                    command_output_details(&output)
                );
                return managed;
            }
            Err(error) => {
                let _ = writeln!(
                    log,
                    "Docker Desktop could not be started; using file-backed FTEP state: {error}"
                );
                return managed;
            }
        }

        if !wait_for_docker_daemon() {
            let _ = writeln!(
                log,
                "Docker Desktop did not become ready within {} seconds; using file-backed FTEP state.",
                DOCKER_DAEMON_TIMEOUT.as_secs()
            );
            return managed;
        }
    }

    match local_database_container_state() {
        Ok(LocalDatabaseContainerState::Running) => {
            let _ = writeln!(
                log,
                "Using the already-running Docker container {LOCAL_FTEP_DATABASE_CONTAINER}."
            );
        }
        Ok(LocalDatabaseContainerState::Stopped) => {
            match run_docker(&["start", LOCAL_FTEP_DATABASE_CONTAINER]) {
                Ok(output) if output.status.success() => {
                    managed.container_started_by_sre = true;
                    let _ = writeln!(
                        log,
                        "Started the Docker container {LOCAL_FTEP_DATABASE_CONTAINER}."
                    );
                }
                Ok(output) => {
                    let _ = writeln!(
                        log,
                        "The Docker container {LOCAL_FTEP_DATABASE_CONTAINER} could not be started; using file-backed FTEP state: {}",
                        command_output_details(&output)
                    );
                }
                Err(error) => {
                    let _ = writeln!(
                        log,
                        "The Docker container {LOCAL_FTEP_DATABASE_CONTAINER} could not be started; using file-backed FTEP state: {error}"
                    );
                }
            }
        }
        Ok(LocalDatabaseContainerState::Missing) => {
            let _ = writeln!(
                log,
                "Docker container {LOCAL_FTEP_DATABASE_CONTAINER} was not found; using file-backed FTEP state."
            );
        }
        Err(error) => {
            let _ = writeln!(
                log,
                "Docker container {LOCAL_FTEP_DATABASE_CONTAINER} could not be inspected; using file-backed FTEP state: {error}"
            );
        }
    }

    managed
}

#[derive(Debug, PartialEq, Eq)]
enum LocalDatabaseContainerState {
    Running,
    Stopped,
    Missing,
}

fn docker_command() -> Command {
    let mut command = Command::new("docker");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn run_docker(args: &[&str]) -> Result<Output, String> {
    docker_command()
        .args(args)
        .output()
        .map_err(|error| format!("could not run Docker CLI: {error}"))
}

fn docker_daemon_is_ready() -> Result<bool, String> {
    Ok(run_docker(&["info", "--format", "{{.ServerVersion}}"])?
        .status
        .success())
}

fn wait_for_docker_daemon() -> bool {
    let deadline = Instant::now() + DOCKER_DAEMON_TIMEOUT;
    while Instant::now() < deadline {
        if docker_daemon_is_ready().unwrap_or(false) {
            return true;
        }
        std::thread::sleep(DOCKER_POLL_INTERVAL);
    }
    false
}

fn local_database_container_state() -> Result<LocalDatabaseContainerState, String> {
    let output = run_docker(&[
        "inspect",
        "--type",
        "container",
        "--format",
        "{{.State.Running}}",
        LOCAL_FTEP_DATABASE_CONTAINER,
    ])?;
    if output.status.success() {
        return match String::from_utf8_lossy(&output.stdout).trim() {
            "true" => Ok(LocalDatabaseContainerState::Running),
            "false" => Ok(LocalDatabaseContainerState::Stopped),
            state => Err(format!(
                "Docker returned an unknown container state: {state}"
            )),
        };
    }

    let details = command_output_details(&output);
    if details.contains("No such object") || details.contains("No such container") {
        Ok(LocalDatabaseContainerState::Missing)
    } else {
        Err(details)
    }
}

fn running_docker_containers() -> Result<bool, String> {
    let output = run_docker(&["ps", "--quiet"])?;
    if !output.status.success() {
        return Err(command_output_details(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| !line.trim().is_empty()))
}

fn command_output_details(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !stdout.is_empty() {
        return stdout;
    }
    format!("Docker exited with {}", output.status)
}

fn local_workspace_root() -> Result<PathBuf, String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    if workspace.join("apps/web/package.json").is_file() {
        Ok(workspace)
    } else {
        Err(
            "FICSIT-0002: Local FTEP source is unavailable. Run the SRE development launcher from its workspace."
                .to_owned(),
        )
    }
}

fn reserve_loopback_port() -> Result<u16, String> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("FICSIT-0002: Cannot reserve a local FTEP port: {error}"))
}

fn stop_local_ftep_process(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// Ends only a process id recorded when SRE launched a game. The caller must
/// look that id up from `active_game_processes`; this helper never scans for
/// or guesses at unrelated applications.
#[cfg(windows)]
fn terminate_owned_game_process(process_id: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("FICSIT-0001: Could not stop the running game: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(format!(
        "FICSIT-0001: Windows could not stop the running game process{details_suffix}.",
        details_suffix = if details.is_empty() {
            String::new()
        } else {
            format!(" ({details})")
        }
    ))
}

#[cfg(not(windows))]
fn terminate_owned_game_process(_process_id: u32) -> Result<(), String> {
    Err("FICSIT-0001: Stopping a running game is currently supported on Windows only.".to_owned())
}

#[cfg(windows)]
fn contain_local_ftep_process(child: &Child) -> Result<LocalFtepJob, String> {
    let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|error| {
        format!("FICSIT-0002: Cannot create the local FTEP process job: {error}")
    })?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let result = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .and_then(|_| AssignProcessToJobObject(job, HANDLE(child.as_raw_handle() as *mut _)))
    };
    if let Err(error) = result {
        unsafe {
            let _ = CloseHandle(job);
        }
        return Err(format!(
            "FICSIT-0002: Cannot contain the local FTEP process tree: {error}"
        ));
    }
    Ok(LocalFtepJob(job.0 as isize))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicDeviceIdentity {
    device_id: String,
    public_key: String,
}

#[tauri::command]
pub(crate) fn device_identity(app: tauri::AppHandle) -> Result<PublicDeviceIdentity, String> {
    let identity = DeviceStore::new(storage::app_data_dir(&app)?.join("device-identity.json"))
        .load_or_create()?;
    Ok(PublicDeviceIdentity {
        device_id: identity.device_id.to_string(),
        public_key: identity.public_key,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerifyLeaseRequest {
    signed_lease: SignedLease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedLease {
    signed_lease: SignedLease,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustedKeySet {
    schema_version: u32,
    keys: Vec<TrustedKey>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustedKey {
    key_id: String,
    algorithm: String,
    public_key: String,
    #[serde(default)]
    development_only: bool,
}

fn decode_public_key(value: &str) -> Result<VerifyingKey, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "FICSIT-0005: FTEP public key encoding is invalid.".to_owned())?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "FICSIT-0005: FTEP public key length is invalid.".to_owned())?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|_| "FICSIT-0005: FTEP public key is invalid.".to_owned())
}

fn trusted_key_paths(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let project_keyset =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/trust/ftep-signing-keys.json");
    app.path()
        .resource_dir()
        .ok()
        .into_iter()
        .flat_map(|directory| {
            [
                directory.join("trust/ftep-signing-keys.json"),
                directory.join("resources/trust/ftep-signing-keys.json"),
            ]
        })
        .chain([project_keyset])
        .collect()
}

fn load_trusted_key(app: &tauri::AppHandle, key_id: &str) -> Result<VerifyingKey, String> {
    for path in trusted_key_paths(app) {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let keyset: TrustedKeySet = serde_json::from_slice(&bytes)
            .map_err(|_| "FICSIT-0005: The bundled FTEP trust set is invalid.".to_owned())?;
        if keyset.schema_version != 1 {
            return Err(
                "FICSIT-0005: The bundled FTEP trust set version is unsupported.".to_owned(),
            );
        }
        if let Some(key) = keyset
            .keys
            .iter()
            .find(|key| key.key_id == key_id && (!key.development_only || cfg!(debug_assertions)))
        {
            if key.algorithm != "Ed25519" {
                return Err("FICSIT-0005: The FTEP signing algorithm is unsupported.".to_owned());
            }
            return decode_public_key(&key.public_key);
        }
    }
    Err(format!(
        "FICSIT-0005: Signing key {key_id} is not trusted by this SRE build. Update SRE before retrying."
    ))
}

fn local_device_id(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(
        DeviceStore::new(storage::app_data_dir(app)?.join("device-identity.json"))
            .load_or_create()?
            .device_id
            .to_string(),
    )
}

fn validate_lease(
    app: &tauri::AppHandle,
    cached: &CachedLease,
    now_unix_secs: u64,
) -> Result<EntitlementLease, String> {
    let key_id = &cached.signed_lease.key_id;
    match LeaseVerifier::new(key_id, load_trusted_key(app, key_id)?).verify(
        &cached.signed_lease,
        &local_device_id(app)?,
        now_unix_secs,
        &BTreeSet::new(),
    ) {
        LeaseStatus::Valid(lease) => Ok(lease),
        LeaseStatus::NotYetValid => {
            Err("FICSIT-0005: Cached entitlement is not yet valid.".to_owned())
        }
        LeaseStatus::Expired => {
            Err("FICSIT-0005: Cached entitlement expired. Reconnect to FTEP.".to_owned())
        }
        LeaseStatus::Revoked => {
            Err("FICSIT-0005: Future FTEP-authorized launches are suspended.".to_owned())
        }
        LeaseStatus::WrongDevice => {
            Err("FICSIT-0005: Cached entitlement belongs to another device.".to_owned())
        }
        LeaseStatus::BadSignature | LeaseStatus::Malformed => {
            Err("FICSIT-0005: Cached entitlement signature is invalid.".to_owned())
        }
    }
}

#[tauri::command]
pub(crate) fn verify_cached_lease(
    app: tauri::AppHandle,
    request: VerifyLeaseRequest,
) -> Result<String, String> {
    let cached = CachedLease {
        signed_lease: request.signed_lease,
    };
    validate_lease(&app, &cached, now_unix_secs()).map(|_| "VALID".to_owned())
}

#[tauri::command]
pub(crate) fn cache_entitlement_lease(
    app: tauri::AppHandle,
    request: VerifyLeaseRequest,
) -> Result<(), String> {
    cache_signed_lease(&app, request.signed_lease).map(|_| ())
}

fn cache_signed_lease(
    app: &tauri::AppHandle,
    signed_lease: SignedLease,
) -> Result<EntitlementLease, String> {
    let cached = CachedLease { signed_lease };
    let lease = validate_lease(app, &cached, now_unix_secs())?;
    storage::atomic_write(
        &storage::app_data_dir(app)?.join("cached-entitlement.json"),
        &serde_json::to_vec_pretty(&cached).map_err(|error| error.to_string())?,
    )?;
    Ok(lease)
}

fn load_cached_lease(app: &tauri::AppHandle) -> Result<CachedLease, String> {
    let path = storage::app_data_dir(app)?.join("cached-entitlement.json");
    serde_json::from_slice(&std::fs::read(path).map_err(|_| {
        "FICSIT-0005: No cached entitlement is available. Connect SRE to FTEP.".to_owned()
    })?)
    .map_err(|_| "FICSIT-0005: Cached entitlement storage is invalid.".to_owned())
}

pub(crate) fn evaluate_cached_achievement(
    app: &tauri::AppHandle,
    overlay: OverlayQueue,
    event: AchievementEvent<'_>,
) -> Result<(), String> {
    let cached = load_cached_lease(app)?;
    let lease = validate_lease(app, &cached, now_unix_secs())?;
    let mut engine = AchievementEngine::open(
        &storage::app_data_dir(app)?.join("achievements.sqlite3"),
        overlay,
    )?;
    engine.evaluate(&lease.subject_id, event).map(|_| ())
}

#[tauri::command]
pub(crate) fn entitlement_status(app: tauri::AppHandle) -> Result<String, String> {
    validate_lease(&app, &load_cached_lease(&app)?, now_unix_secs()).map(|_| "VALID".to_owned())
}

#[tauri::command]
pub(crate) fn session_history(app: tauri::AppHandle) -> Result<SessionHistory, String> {
    SessionStore::new(storage::app_data_dir(&app)?.join("sessions.json"))
        .load()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn achievement_count(
    app: tauri::AppHandle,
    services: tauri::State<'_, LocalServices>,
) -> Result<usize, String> {
    AchievementEngine::open(
        &storage::app_data_dir(&app)?.join("achievements.sqlite3"),
        services.overlay.clone(),
    )?
    .unlocked_count()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BeginConnectionRequest {
    web_base_url: Option<String>,
}

#[tauri::command]
pub(crate) fn ftep_web_url(services: tauri::State<'_, LocalServices>) -> Result<String, String> {
    services.managed_local_ftep_url()
}

#[tauri::command]
pub(crate) fn begin_ftep_connection(
    app: tauri::AppHandle,
    services: tauri::State<'_, LocalServices>,
    request: BeginConnectionRequest,
) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
        format!("FICSIT-0002: Cannot start the local sign-in callback: {error}")
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("FICSIT-0002: Cannot configure sign-in callback: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("FICSIT-0002: Cannot inspect sign-in callback: {error}"))?
        .port();
    let identity = DeviceStore::new(storage::app_data_dir(&app)?.join("device-identity.json"))
        .load_or_create()?;
    let treaty = crate::treaty_metadata()?;
    let state = uuid::Uuid::new_v4().to_string();
    let callback = format!("http://127.0.0.1:{port}/callback");
    let web_base_url = match request.web_base_url {
        Some(web_base_url) => web_base_url,
        None => services.managed_local_ftep_url()?,
    };
    let mut url = Url::parse(&web_base_url)
        .map_err(|_| "FICSIT-0002: The configured FTEP web URL is invalid.".to_owned())?;
    let local_development =
        url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1") | Some("localhost"));
    if !(url.scheme() == "https" || local_development)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("FICSIT-0002: FTEP sign-in requires HTTPS (or local development).".to_owned());
    }
    url.set_path("/connect");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("deviceId", &identity.device_id.to_string())
        .append_pair("devicePublicKey", &identity.public_key)
        .append_pair("callback", &callback)
        .append_pair("state", &state)
        .append_pair("accordVersion", &treaty.version)
        .append_pair("accordHash", &treaty.sha256);

    let app_for_callback = app.clone();
    let services_for_callback = services.inner().clone();
    std::thread::spawn(move || {
        receive_ftep_callback(listener, &app_for_callback, &services_for_callback, &state);
    });
    Ok(url.into())
}

fn receive_ftep_callback(
    listener: TcpListener,
    app: &tauri::AppHandle,
    services: &LocalServices,
    expected_state: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if handle_callback_stream(&mut stream, app, services, expected_state) {
                    return;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

fn handle_callback_stream(
    stream: &mut TcpStream,
    app: &tauri::AppHandle,
    services: &LocalServices,
    expected_state: &str,
) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = [0_u8; 16_384];
    let Ok(size) = stream.read(&mut request) else {
        respond_callback(stream, false);
        return false;
    };
    let first_line = String::from_utf8_lossy(&request[..size])
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let Some(target) = first_line.split_whitespace().nth(1) else {
        respond_callback(stream, false);
        return false;
    };
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        respond_callback(stream, false);
        return false;
    };
    if url.path() != "/callback" {
        respond_callback(stream, false);
        return false;
    }
    let parameters: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    if parameters.get("state").map(String::as_str) != Some(expected_state) {
        respond_callback(stream, false);
        return false;
    }
    let result = parameters
        .get("lease")
        .ok_or_else(|| "missing lease".to_owned())
        .and_then(|encoded| {
            URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| "invalid lease".to_owned())
        })
        .and_then(|bytes| {
            serde_json::from_slice::<SignedLease>(&bytes).map_err(|_| "invalid lease".to_owned())
        })
        .and_then(|signed| cache_signed_lease(app, signed));
    let Ok(_lease) = result else {
        respond_callback(stream, false);
        return true;
    };
    let at = now_unix_secs().saturating_mul(1_000);
    for event in [
        AchievementEvent::OAuthCompleted { at_unix_ms: at },
        AchievementEvent::DeviceRegistered { at_unix_ms: at },
        AchievementEvent::TreatyRatified { at_unix_ms: at },
        AchievementEvent::ArticleIIActivated { at_unix_ms: at },
        AchievementEvent::BackendMigrationApplied { at_unix_ms: at },
        AchievementEvent::ControlPlaneHealthy { at_unix_ms: at },
    ] {
        let _ = evaluate_cached_achievement(app, services.overlay.clone(), event);
    }
    let _ = ftep_client::sync_pending_achievements(
        &services.managed_local_ftep_url().unwrap_or_default(),
        &storage::app_data_dir(app).unwrap_or_else(|_| std::env::temp_dir().join("SRE")),
    );
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.show();
    }
    respond_callback(stream, true);
    true
}

fn respond_callback(stream: &mut TcpStream, success: bool) {
    let (status, heading, body) = if success {
        (
            "200 OK",
            "SRE connected",
            "The signed entitlement is cached. You can close this tab and return to SRE.",
        )
    } else {
        (
            "400 Bad Request",
            "Connection rejected",
            "Return to SRE and start the browser connection again.",
        )
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><meta http-equiv=Content-Security-Policy content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{heading}</title><main style=\"font-family:system-ui;max-width:42rem;margin:10vh auto;padding:2rem;background:#10201f;color:#edf4f2\"><h1>{heading}</h1><p>{body}</p></main>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

pub(crate) fn authorized_account(
    app: &tauri::AppHandle,
    game_id: &str,
    franchise: &str,
) -> Result<String, String> {
    let lease = validate_lease(app, &load_cached_lease(app)?, now_unix_secs())?;
    if !allows_game(&lease, game_id, franchise) {
        return Err("FICSIT-0005: The cached lease does not authorize this title.".to_owned());
    }
    Ok(lease.subject_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchRegisteredRequest {
    installation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StopRunningGameRequest {
    session_id: String,
}

#[tauri::command]
pub(crate) fn cancel_registered_game_launch(
    services: tauri::State<'_, LocalServices>,
) -> Result<(), String> {
    services.cancel_game_launch()
}

#[tauri::command]
pub(crate) fn stop_running_game(
    services: tauri::State<'_, LocalServices>,
    request: StopRunningGameRequest,
) -> Result<(), String> {
    services.stop_game_process(&request.session_id)
}

#[tauri::command]
pub(crate) fn active_running_game_sessions(
    services: tauri::State<'_, LocalServices>,
) -> Result<Vec<String>, String> {
    services.active_game_session_ids()
}

#[tauri::command]
pub(crate) async fn launch_registered_game(
    app: tauri::AppHandle,
    services: tauri::State<'_, LocalServices>,
    request: LaunchRegisteredRequest,
) -> Result<RuntimeSession, String> {
    let services = services.inner().clone();
    let cancellation = services.begin_game_launch()?;
    let worker_services = services.clone();
    let worker_cancellation = cancellation.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        launch_registered_game_blocking(app, worker_services, request, &worker_cancellation)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("FICSIT-0001: Game launch task failed: {error}")),
    };
    services.finish_game_launch(&cancellation);
    result
}

fn launch_registered_game_blocking(
    app: tauri::AppHandle,
    services: LocalServices,
    request: LaunchRegisteredRequest,
    cancellation: &AtomicBool,
) -> Result<RuntimeSession, String> {
    let data_dir = storage::app_data_dir(&app)?;
    let state = LibraryStore::new(data_dir.join("library.json"))
        .load()
        .map_err(|error| error.to_string())?;
    let installation = state
        .installations
        .get(&request.installation_id)
        .ok_or_else(|| "FICSIT-0007: The selected installation no longer exists.".to_owned())?
        .clone();
    let account_id = authorized_account(&app, installation.game_id.as_str(), "nintendo")?;

    if cancellation.load(Ordering::SeqCst) {
        return Err("FICSIT-0001: Game launch was cancelled.".to_owned());
    }
    let mut values = BTreeMap::new();
    values.insert("device_id".to_owned(), local_device_id(&app)?);
    let launch_request = LaunchRequest {
        session_id: uuid::Uuid::new_v4().to_string(),
        installation: GameInstallation {
            installation_id: installation.installation_id,
            game_id: installation.game_id.clone(),
            variant_id: installation.variant_id,
            runtime_id: installation.runtime_id.clone(),
            root: installation.game_source,
            synthetic_fixture: false,
        },
        config: RuntimeConfig { values },
        mode: LaunchMode::Authorized,
    };
    let (provider, session): (Arc<dyn RuntimeProvider>, RuntimeSession) =
        match installation.runtime_id.as_str() {
            "shipwright" => {
                let shipwright = Arc::new(crate::importer::shipwright_adapter(&app));
                let session = shipwright
                    .launch_cancellable(launch_request, cancellation)
                    .map_err(|error| error.to_string())?;
                (shipwright, session)
            }
            "two-ship" => {
                let two_ship = Arc::new(crate::importer::two_ship_adapter(&app)?);
                let session = two_ship
                    .launch(launch_request)
                    .map_err(|error| error.to_string())?;
                (two_ship, session)
            }
            "cemu-compatible" => {
                let provider = Arc::new(WiiURuntimeAdapter::new(
                    installation.runtime_executable.clone(),
                ));
                let session = provider
                    .launch(launch_request)
                    .map_err(|error| error.to_string())?;
                (provider, session)
            }
            "switch-runtime" => {
                let provider = Arc::new(SwitchRuntimeProvider::new(Some(
                    ExternalSwitchImplementation::manually_configured(
                        "managed-ryujinx-canary",
                        "Managed Ryujinx Canary runtime",
                        crate::importer::managed_ryujinx_executable(&app)?,
                    ),
                )));
                let session = provider
                    .launch(launch_request)
                    .map_err(|error| error.to_string())?;
                (provider, session)
            }
            "dolphin-compatible" => {
                let provider = Arc::new(DolphinRuntimeAdapter::new(
                    installation.runtime_executable.clone(),
                ));
                let session = provider
                    .launch(launch_request)
                    .map_err(|error| error.to_string())?;
                (provider, session)
            }
            _ => return Err("FICSIT-0008: This runtime has no launch adapter.".to_owned()),
        };

    if cancellation.load(Ordering::SeqCst) {
        if let Some(process_id) = session.process_id {
            let _ = terminate_owned_game_process(process_id);
        }
        return Err("FICSIT-0001: Game launch was cancelled.".to_owned());
    }

    let store = SessionStore::new(data_dir.join("sessions.json"));
    {
        let _guard = services
            .session_write
            .lock()
            .map_err(|_| "FICSIT-0001: Session storage is unavailable.".to_owned())?;
        store
            .upsert(session.clone())
            .map_err(|error| error.to_string())?;
    }
    spawn_ftep_sync(&app, &services, &session);
    if let Some(process_id) = session.process_id {
        services.track_game_process(&session.session_id, process_id);
    }
    if let Ok(mut engine) = AchievementEngine::open(
        &data_dir.join("achievements.sqlite3"),
        services.overlay.clone(),
    ) {
        let _ = engine.evaluate(
            &account_id,
            AchievementEvent::GameLaunched {
                game_id: &session.game_id,
                at_unix_ms: session
                    .started_at_unix_ms
                    .unwrap_or(session.requested_at_unix_ms),
            },
        );
    }
    monitor_session(
        app,
        services,
        provider,
        session.clone(),
        account_id,
        data_dir,
    );
    Ok(session)
}

fn monitor_session(
    app: tauri::AppHandle,
    services: LocalServices,
    provider: Arc<dyn RuntimeProvider>,
    mut session: RuntimeSession,
    account_id: String,
    data_dir: PathBuf,
) {
    std::thread::spawn(move || {
        let store = SessionStore::new(data_dir.join("sessions.json"));
        let managed_session_id = session.session_id.clone();
        let mut playable_recorded = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let observation = match provider.observe_process(&session) {
                Ok(value) => value,
                Err(error) => {
                    session.state = RuntimeSessionState::Failed;
                    session.launch_result = format!("OBSERVATION_FAILED:{:?}", error.code);
                    if let Ok(_guard) = services.session_write.lock() {
                        let _ = store.upsert(session.clone());
                    }
                    spawn_ftep_sync(&app, &services, &session);
                    services.release_game_process(&managed_session_id);
                    return;
                }
            };
            if !playable_recorded
                && let Ok(playable) = provider.determine_playable_state(&session)
                && playable.playable
            {
                playable_recorded = true;
                session.state = RuntimeSessionState::Playable;
                session.playable_at_unix_ms = Some(playable.observed_at_unix_ms);
                session.launch_result = match playable.precision {
                    PlayablePrecision::Exact => "PLAYABLE_EXACT",
                    PlayablePrecision::Approximate => "PLAYABLE_APPROXIMATE",
                }
                .to_owned();
                if let Ok(mut engine) = AchievementEngine::open(
                    &data_dir.join("achievements.sqlite3"),
                    services.overlay.clone(),
                ) {
                    let event = RuntimeEvent::Playable {
                        at_unix_ms: playable.observed_at_unix_ms,
                        precision: playable.precision,
                    };
                    if engine
                        .evaluate(
                            &account_id,
                            AchievementEvent::Runtime {
                                game_id: &session.game_id,
                                event: &event,
                            },
                        )
                        .is_ok()
                        && let Some(window) = app.get_webview_window("overlay")
                    {
                        let _ = window.show();
                    }
                }
                if let Ok(_guard) = services.session_write.lock() {
                    let _ = store.upsert(session.clone());
                }
                spawn_ftep_sync(&app, &services, &session);
            }
            if !observation.running {
                let ended = observation.observed_at_unix_ms;
                session.state = RuntimeSessionState::Ended;
                session.ended_at_unix_ms = Some(ended);
                session.exit_code = observation.exit_code;
                session.duration_ms = session
                    .started_at_unix_ms
                    .map(|start| ended.saturating_sub(start));
                session.launch_result = if observation.exit_code.unwrap_or(0) == 0 {
                    "ENDED"
                } else {
                    "CRASHED"
                }
                .to_owned();
                if let Ok(_guard) = services.session_write.lock() {
                    let _ = store.upsert(session.clone());
                }
                spawn_ftep_sync(&app, &services, &session);
                services.release_game_process(&managed_session_id);
                return;
            }
        }
    });
}

fn spawn_ftep_sync(app: &tauri::AppHandle, services: &LocalServices, session: &RuntimeSession) {
    let Ok(base_url) = services.managed_local_ftep_url() else {
        return;
    };
    let Ok(data_dir) = storage::app_data_dir(app) else {
        return;
    };
    let session = session.clone();
    std::thread::spawn(move || {
        let _ = ftep_client::sync_session(&base_url, &data_dir, &session);
        let _ = ftep_client::sync_pending_achievements(&base_url, &data_dir);
    });
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub(crate) fn run_sre_doctor(
    app: tauri::AppHandle,
    runtime_executable: Option<String>,
) -> Result<DoctorReport, String> {
    Ok(run_doctor(
        &storage::app_data_dir(&app)?,
        runtime_executable.as_deref().map(Path::new),
    ))
}

#[tauri::command]
pub(crate) fn export_diagnostics(app: tauri::AppHandle, destination: String) -> Result<(), String> {
    export_sanitized(
        &run_doctor(&storage::app_data_dir(&app)?, None),
        &PathBuf::from(destination),
    )
}

#[tauri::command]
pub(crate) fn pop_overlay(
    services: tauri::State<'_, LocalServices>,
) -> Result<Option<OverlayNotification>, String> {
    services.overlay.pop()
}

#[tauri::command]
pub(crate) fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}
