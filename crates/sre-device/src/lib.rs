//! Privacy-preserving device identity: a random UUID and a local Ed25519 key.
//! No hardware fingerprint, serial number, MAC address, or telemetry is used.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceIdentity {
    pub device_id: Uuid,
    pub public_key: String,
    private_key: String,
    pub created_at_unix_ms: u64,
}

impl DeviceIdentity {
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self {
            device_id: Uuid::new_v4(),
            public_key: URL_SAFE_NO_PAD.encode(signing.verifying_key().as_bytes()),
            private_key: URL_SAFE_NO_PAD.encode(signing.to_bytes()),
            created_at_unix_ms: now_unix_ms(),
        }
    }

    pub fn signing_key(&self) -> Result<SigningKey, String> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.private_key)
            .map_err(|_| "device private key encoding is invalid")?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "device private key length is invalid")?;
        let signing = SigningKey::from_bytes(&bytes);
        if URL_SAFE_NO_PAD.encode(signing.verifying_key().as_bytes()) != self.public_key {
            return Err("device public and private keys do not match".to_owned());
        }
        Ok(signing)
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, String> {
        Ok(self.signing_key()?.verifying_key())
    }
}

pub struct DeviceStore {
    path: PathBuf,
}

impl DeviceStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load_or_create(&self) -> Result<DeviceIdentity, String> {
        if self.path.exists() {
            let identity: DeviceIdentity =
                serde_json::from_slice(&fs::read(&self.path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            identity.signing_key()?;
            return Ok(identity);
        }
        let identity = DeviceIdentity::generate();
        let parent = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        temporary
            .write_all(&serde_json::to_vec_pretty(&identity).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        temporary
            .persist(&self.path)
            .map_err(|error| error.error.to_string())?;
        Ok(identity)
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_random_and_persists_without_fingerprinting() {
        let temp = tempfile::tempdir().unwrap();
        let store = DeviceStore::new(temp.path().join("device.json"));
        let first = store.load_or_create().unwrap();
        let second = store.load_or_create().unwrap();
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(first.public_key, second.public_key);
        assert_ne!(first.device_id, DeviceIdentity::generate().device_id);
    }
}
