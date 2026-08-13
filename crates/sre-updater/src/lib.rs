//! The updater verifies signed metadata and file hashes. It returns a verified
//! candidate path; process execution and installer consent remain UI concerns.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: Url,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub product: String,
    pub version: String,
    pub channel: ReleaseChannel,
    pub published_at: String,
    pub assets: Vec<ReleaseAsset>,
}

pub struct ManifestVerifier {
    public_key: VerifyingKey,
}

impl ManifestVerifier {
    pub fn new(public_key: VerifyingKey) -> Self {
        Self { public_key }
    }

    pub fn verify_manifest(
        &self,
        bytes: &[u8],
        signature_base64: &str,
    ) -> Result<ReleaseManifest, String> {
        let signature = STANDARD
            .decode(signature_base64.trim())
            .ok()
            .and_then(|value| Signature::from_slice(&value).ok())
            .ok_or_else(|| "release manifest signature encoding is invalid".to_owned())?;
        self.public_key
            .verify(bytes, &signature)
            .map_err(|_| "release manifest signature is invalid".to_owned())?;
        let manifest: ReleaseManifest = serde_json::from_slice(bytes)
            .map_err(|error| format!("release manifest is invalid: {error}"))?;
        validate_manifest(&manifest)?;
        Ok(manifest)
    }

    pub fn verify_download(&self, asset: &ReleaseAsset, path: &Path) -> Result<(), String> {
        if path.metadata().map_err(|error| error.to_string())?.len() != asset.size {
            return Err("download size does not match the signed manifest".to_owned());
        }
        let file = File::open(path).map_err(|error| error.to_string())?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let actual = format!("{:x}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(&asset.sha256) {
            return Err("download SHA-256 does not match the signed manifest".to_owned());
        }
        Ok(())
    }
}

fn validate_manifest(manifest: &ReleaseManifest) -> Result<(), String> {
    if manifest.schema_version != 1
        || manifest.product != "SRE"
        || manifest.version.trim().is_empty()
    {
        return Err("release manifest identity is unsupported".to_owned());
    }
    if manifest.assets.is_empty() {
        return Err("release manifest has no assets".to_owned());
    }
    for asset in &manifest.assets {
        if !matches!(
            asset.name.as_str(),
            "SRE-Setup-x64.exe" | "SRE-Portable-x64.zip" | "SHA256SUMS.txt"
        ) {
            return Err(format!(
                "release asset {} is not on the executable allowlist",
                asset.name
            ));
        }
        if asset.url.scheme() != "https"
            || !matches!(
                asset.url.host_str(),
                Some("github.com" | "objects.githubusercontent.com")
            )
        {
            return Err(format!(
                "release asset {} does not use an approved GitHub HTTPS host",
                asset.name
            ));
        }
        if asset.sha256.len() != 64 || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "release asset {} has an invalid SHA-256",
                asset.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use std::fs;

    #[test]
    fn verifies_signature_hash_and_rejects_arbitrary_hosts() {
        let temp = tempfile::tempdir().unwrap();
        let download = temp.path().join("SRE-Setup-x64.exe");
        fs::write(&download, b"installer fixture").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"installer fixture"));
        let mut manifest = ReleaseManifest {
            schema_version: 1,
            product: "SRE".to_owned(),
            version: "0.1.0-rc.1".to_owned(),
            channel: ReleaseChannel::Stable,
            published_at: "2026-08-13T00:00:00Z".to_owned(),
            assets: vec![ReleaseAsset {
                name: "SRE-Setup-x64.exe".to_owned(),
                url: Url::parse(
                    "https://github.com/example/sre/releases/download/v0.1/SRE-Setup-x64.exe",
                )
                .unwrap(),
                sha256: hash,
                size: 17,
            }],
        };
        let key = SigningKey::generate(&mut OsRng);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = STANDARD.encode(key.sign(&bytes).to_bytes());
        let verifier = ManifestVerifier::new(key.verifying_key());
        let verified = verifier.verify_manifest(&bytes, &signature).unwrap();
        verifier
            .verify_download(&verified.assets[0], &download)
            .unwrap();
        manifest.assets[0].url = Url::parse("https://example.com/setup.exe").unwrap();
        let unsafe_bytes = serde_json::to_vec(&manifest).unwrap();
        let unsafe_signature = STANDARD.encode(key.sign(&unsafe_bytes).to_bytes());
        assert!(
            verifier
                .verify_manifest(&unsafe_bytes, &unsafe_signature)
                .is_err()
        );
    }
}
