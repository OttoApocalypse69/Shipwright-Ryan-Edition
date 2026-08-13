//! SRE contains only lease verification. FTEP signing keys belong exclusively
//! to the server/control-plane deployment.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntitlementLease {
    pub lease_id: String,
    pub subject_id: String,
    pub device_id: String,
    pub entitlements: BTreeSet<String>,
    pub issued_at_unix_secs: u64,
    pub not_before_unix_secs: u64,
    pub expires_at_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedLease {
    pub payload: String,
    pub signature: String,
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseStatus {
    Valid(EntitlementLease),
    NotYetValid,
    Expired,
    Revoked,
    WrongDevice,
    BadSignature,
    Malformed,
}

pub struct LeaseVerifier {
    key_id: String,
    public_key: VerifyingKey,
}

impl LeaseVerifier {
    pub fn new(key_id: impl Into<String>, public_key: VerifyingKey) -> Self {
        Self {
            key_id: key_id.into(),
            public_key,
        }
    }

    pub fn verify(
        &self,
        signed: &SignedLease,
        expected_device_id: &str,
        now_unix_secs: u64,
        revoked_lease_ids: &BTreeSet<String>,
    ) -> LeaseStatus {
        if signed.key_id != self.key_id {
            return LeaseStatus::BadSignature;
        }
        let payload = match URL_SAFE_NO_PAD.decode(&signed.payload) {
            Ok(value) => value,
            Err(_) => return LeaseStatus::Malformed,
        };
        let signature = match URL_SAFE_NO_PAD
            .decode(&signed.signature)
            .ok()
            .and_then(|value| Signature::from_slice(&value).ok())
        {
            Some(value) => value,
            None => return LeaseStatus::Malformed,
        };
        if self.public_key.verify(&payload, &signature).is_err() {
            return LeaseStatus::BadSignature;
        }
        let lease: EntitlementLease = match serde_json::from_slice(&payload) {
            Ok(value) => value,
            Err(_) => return LeaseStatus::Malformed,
        };
        if revoked_lease_ids.contains(&lease.lease_id) {
            return LeaseStatus::Revoked;
        }
        if lease.device_id != expected_device_id {
            return LeaseStatus::WrongDevice;
        }
        if now_unix_secs < lease.not_before_unix_secs {
            return LeaseStatus::NotYetValid;
        }
        if now_unix_secs >= lease.expires_at_unix_secs {
            return LeaseStatus::Expired;
        }
        LeaseStatus::Valid(lease)
    }
}

pub fn allows_game(lease: &EntitlementLease, game_id: &str, franchise: &str) -> bool {
    lease.entitlements.contains("ftep.library")
        || lease
            .entitlements
            .contains(&format!("ftep.library.{franchise}"))
        || lease.entitlements.contains(&format!("ftep.game.{game_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn signed_lease(device: &str, start: u64, end: u64) -> (SignedLease, LeaseVerifier) {
        let key = SigningKey::generate(&mut OsRng);
        let lease = EntitlementLease {
            lease_id: "lease-1".to_owned(),
            subject_id: "account-1".to_owned(),
            device_id: device.to_owned(),
            entitlements: BTreeSet::from(["ftep.library.nintendo".to_owned()]),
            issued_at_unix_secs: start,
            not_before_unix_secs: start,
            expires_at_unix_secs: end,
        };
        let payload = serde_json::to_vec(&lease).unwrap();
        let signed = SignedLease {
            payload: URL_SAFE_NO_PAD.encode(&payload),
            signature: URL_SAFE_NO_PAD.encode(key.sign(&payload).to_bytes()),
            key_id: "test-key".to_owned(),
        };
        (signed, LeaseVerifier::new("test-key", key.verifying_key()))
    }

    #[test]
    fn offline_valid_expired_revoked_wrong_device_and_bad_signature() {
        let (signed, verifier) = signed_lease("device-1", 100, 200);
        assert!(matches!(
            verifier.verify(&signed, "device-1", 150, &BTreeSet::new()),
            LeaseStatus::Valid(_)
        ));
        assert_eq!(
            verifier.verify(&signed, "device-1", 200, &BTreeSet::new()),
            LeaseStatus::Expired
        );
        assert_eq!(
            verifier.verify(&signed, "device-2", 150, &BTreeSet::new()),
            LeaseStatus::WrongDevice
        );
        assert_eq!(
            verifier.verify(
                &signed,
                "device-1",
                150,
                &BTreeSet::from(["lease-1".to_owned()])
            ),
            LeaseStatus::Revoked
        );
        let mut corrupt = signed;
        corrupt.signature.replace_range(..2, "AA");
        assert_eq!(
            verifier.verify(&corrupt, "device-1", 150, &BTreeSet::new()),
            LeaseStatus::BadSignature
        );
    }

    #[test]
    fn policy_eligibility_is_separate_from_technical_compatibility() {
        let (_, verifier) = signed_lease("device-1", 100, 200);
        let (signed, _) = signed_lease("device-1", 100, 200);
        let _ = verifier;
        let payload = URL_SAFE_NO_PAD.decode(signed.payload).unwrap();
        let lease: EntitlementLease = serde_json::from_slice(&payload).unwrap();
        assert!(allows_game(&lease, "zelda-botw", "nintendo"));
    }
}
