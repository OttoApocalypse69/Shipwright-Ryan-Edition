use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::Signer;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sre_achievements::AchievementEngine;
use sre_device::{DeviceIdentity, DeviceStore};
use sre_runtime::{PlayablePrecision, RuntimeSession, RuntimeSessionState};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub fn sync_session(
    base_url: &str,
    data_dir: &Path,
    session: &RuntimeSession,
) -> Result<(), String> {
    let identity = load_identity(data_dir)?;
    let device_id = session
        .device_id
        .clone()
        .unwrap_or_else(|| identity.device_id.to_string());
    if device_id != identity.device_id.to_string() {
        return Err("session device identity does not match the local device".to_owned());
    }
    let body = json!({
        "sessionId": session.session_id,
        "deviceId": device_id,
        "gameId": session.game_id.as_str(),
        "variantId": session.variant_id.as_str(),
        "runtimeId": session.runtime_id.as_str(),
        "state": session_state(session.state),
        "requestedAt": session.requested_at_unix_ms,
        "startedAt": session.started_at_unix_ms,
        "playableAt": session.playable_at_unix_ms,
        "endedAt": session.ended_at_unix_ms,
        "durationMs": session.duration_ms,
        "playablePrecision": playable_precision(session.initial_events.iter().find_map(|event| match event {
            sre_runtime::RuntimeEvent::Playable { precision, .. } => Some(*precision),
            _ => None,
        })),
        "playableMethod": session.launch_result,
        "exitCode": session.exit_code,
        "launchResult": session.launch_result,
    });
    post_signed(base_url, "/api/sessions", &identity, body).map(|_| ())
}

pub fn sync_pending_achievements(base_url: &str, data_dir: &Path) -> Result<(), String> {
    let identity = load_identity(data_dir)?;
    let database_path = data_dir.join("achievements.sqlite3");
    let engine = AchievementEngine::open(&database_path, Default::default())?;
    let pending = engine.pending_sync(100)?;
    if pending.is_empty() {
        return Ok(());
    }
    let unlocks: Vec<Value> = pending
        .iter()
        .map(|unlock| {
            json!({
                "achievementId": map_achievement_id(&unlock.achievement_id),
                "unlockId": format!("{}:{}", unlock.account_id, unlock.achievement_id),
                "unlockedAtUnixMs": unlock.unlocked_at_unix_ms,
            })
        })
        .collect();
    let body = json!({ "deviceId": identity.device_id, "unlocks": unlocks });
    post_signed(base_url, "/api/achievements/sync", &identity, body)?;
    for unlock in pending {
        engine.mark_synced(&unlock.account_id, &unlock.achievement_id)?;
    }
    Ok(())
}

fn load_identity(data_dir: &Path) -> Result<DeviceIdentity, String> {
    DeviceStore::new(data_dir.join("device-identity.json")).load_or_create()
}

fn post_signed(
    base_url: &str,
    path: &str,
    identity: &DeviceIdentity,
    body: Value,
) -> Result<Value, String> {
    let mut endpoint = url::Url::parse(base_url).map_err(|_| "FTEP URL is invalid".to_owned())?;
    let base_path = endpoint.path().trim_end_matches('/').to_owned();
    endpoint.set_path(&format!("{base_path}{path}"));
    endpoint.set_query(None);
    let local = endpoint.scheme() == "http"
        && matches!(endpoint.host_str(), Some("127.0.0.1") | Some("localhost"));
    if endpoint.scheme() != "https" && !local {
        return Err("FTEP synchronization requires HTTPS".to_owned());
    }
    let body = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let timestamp = now_unix_secs();
    let nonce = Uuid::new_v4().to_string();
    let body_hash = hex_digest(body.as_bytes());
    let message = format!(
        "POST\n{}\n{}\n{}\n{}",
        endpoint.path(),
        timestamp,
        nonce,
        body_hash
    );
    let signature =
        URL_SAFE_NO_PAD.encode(identity.signing_key()?.sign(message.as_bytes()).to_bytes());
    let response = Client::builder()
        .user_agent("SRE-FTEP-Sync/1")
        .build()
        .map_err(|error| error.to_string())?
        .post(endpoint)
        .header("content-type", "application/json")
        .header("x-ftep-device-id", identity.device_id.to_string())
        .header("x-ftep-device-timestamp", timestamp.to_string())
        .header("x-ftep-device-nonce", nonce)
        .header("x-ftep-device-signature", signature)
        .body(body)
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let value = response.json::<Value>().unwrap_or_else(|_| json!({}));
    if !status.is_success() {
        let message = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("FTEP rejected synchronization");
        return Err(format!("{message} ({})", status.as_u16()));
    }
    Ok(value)
}

fn session_state(state: RuntimeSessionState) -> &'static str {
    match state {
        RuntimeSessionState::Requested => "REQUESTED",
        RuntimeSessionState::Authorizing => "AUTHORIZING",
        RuntimeSessionState::Preparing => "PREPARING",
        RuntimeSessionState::StartingRuntime => "STARTING_RUNTIME",
        RuntimeSessionState::RuntimeStarted => "RUNTIME_STARTED",
        RuntimeSessionState::Playable => "PLAYABLE",
        RuntimeSessionState::Ending => "ENDING",
        RuntimeSessionState::Ended => "ENDED",
        RuntimeSessionState::Failed => "FAILED",
    }
}

fn playable_precision(precision: Option<PlayablePrecision>) -> Option<&'static str> {
    precision.map(|value| match value {
        PlayablePrecision::Exact => "PLAYABLE_EXACT",
        PlayablePrecision::Approximate => "PLAYABLE_APPROXIMATE",
    })
}

fn map_achievement_id(id: &str) -> &str {
    match id {
        "ahh-zelda" => "FTEP-ACH-0001",
        "treaty-ratified" => "FTEP-ACH-0002",
        "somehow-this-needed-oauth" => "FTEP-ACH-0003",
        "legally-supplied-bits" => "FTEP-ACH-0004",
        "registered-gaming-apparatus" => "FTEP-ACH-0005",
        "the-invoice-has-come-due" => "FTEP-ACH-0006",
        "ficsit-employee-onboarding" => "FTEP-ACH-0007",
        "article-ii-enjoyer" => "FTEP-ACH-0008",
        "diplomatic-relations-restored" => "FTEP-ACH-0009",
        "postgresql-was-necessary" => "FTEP-ACH-0010",
        "enterprise-gaming" => "FTEP-ACH-0011",
        "ryan-moment" => "FTEP-ACH-0012",
        "fluid-logistics" => "FTEP-ACH-0013",
        "pipeline-operational" => "FTEP-ACH-0014",
        "that-is-not-zelda" => "FTEP-ACH-0015",
        _ => id,
    }
}

fn hex_digest(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}
