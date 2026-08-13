use crate::storage;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use sre_achievements::{AchievementEngine, AchievementEvent};
use sre_device::DeviceStore;
use sre_diagnostics::{DoctorReport, export_sanitized, run_doctor};
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
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;
use url::Url;

#[derive(Clone, Default)]
pub(crate) struct LocalServices {
    pub(crate) overlay: OverlayQueue,
    session_write: Arc<Mutex<()>>,
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
        if let Some(key) = keyset.keys.iter().find(|key| key.key_id == key_id) {
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
    web_base_url: String,
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
    let mut url = Url::parse(&request.web_base_url)
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
    let Ok(lease) = result else {
        respond_callback(stream, false);
        return true;
    };
    if let Ok(mut engine) = AchievementEngine::open(
        &storage::app_data_dir(app)
            .unwrap_or_else(|_| std::env::temp_dir().join("SRE"))
            .join("achievements.sqlite3"),
        services.overlay.clone(),
    ) {
        let at = now_unix_secs().saturating_mul(1_000);
        let _ = engine.evaluate(
            &lease.subject_id,
            AchievementEvent::OAuthCompleted { at_unix_ms: at },
        );
        let _ = engine.evaluate(
            &lease.subject_id,
            AchievementEvent::DeviceRegistered { at_unix_ms: at },
        );
        let _ = engine.evaluate(
            &lease.subject_id,
            AchievementEvent::TreatyRatified { at_unix_ms: at },
        );
    }
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

#[tauri::command]
pub(crate) fn launch_registered_game(
    app: tauri::AppHandle,
    services: tauri::State<'_, LocalServices>,
    request: LaunchRegisteredRequest,
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

    let provider: Arc<dyn RuntimeProvider> = match installation.runtime_id.as_str() {
        "cemu-compatible" => Arc::new(WiiURuntimeAdapter::new(
            installation.runtime_executable.clone(),
        )),
        "switch-runtime" => Arc::new(SwitchRuntimeProvider::new(
            installation.runtime_executable.clone().map(|path| {
                ExternalSwitchImplementation::manually_configured(
                    "user-selected",
                    "User-selected Switch runtime",
                    path,
                )
            }),
        )),
        _ => {
            return Err(
                "FICSIT-0008: Use the native guided launch path for this runtime.".to_owned(),
            );
        }
    };
    let mut values = BTreeMap::new();
    values.insert("device_id".to_owned(), local_device_id(&app)?);
    let session = provider
        .launch(LaunchRequest {
            session_id: uuid::Uuid::new_v4().to_string(),
            installation: GameInstallation {
                installation_id: installation.installation_id,
                game_id: installation.game_id,
                variant_id: installation.variant_id,
                runtime_id: installation.runtime_id,
                root: installation.game_source,
                synthetic_fixture: false,
            },
            config: RuntimeConfig { values },
            mode: LaunchMode::Authorized,
        })
        .map_err(|error| error.to_string())?;

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
        services.inner().clone(),
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
        let mut playable_recorded = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let observation = match provider.observe_process(&session) {
                Ok(value) => value,
                Err(error) => {
                    session.state = RuntimeSessionState::Failed;
                    session.launch_result = format!("OBSERVATION_FAILED:{:?}", error.code);
                    if let Ok(_guard) = services.session_write.lock() {
                        let _ = store.upsert(session);
                    }
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
                    let _ = store.upsert(session);
                }
                return;
            }
        }
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
