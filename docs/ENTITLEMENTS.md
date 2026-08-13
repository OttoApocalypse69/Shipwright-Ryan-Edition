# Devices, entitlements, and signed leases

SRE generates a random UUID and Ed25519 keypair locally. It does not derive identity from hardware. Only the UUID and public key are sent during authenticated device registration.

FTEP issues short-lived Ed25519 leases containing lease ID, subject ID, device ID, entitlement strings, key ID, issue/not-before/expiry times, payload, and signature. Typical grants are `ftep.library`, `ftep.library.nintendo`, or `ftep.game.<game-id>`.

The desktop starts a one-use loopback listener on random `127.0.0.1` port and opens `/connect` in the system browser. Auth.js authenticates there; the user ratifies the canonical Accord; FTEP registers the random device public key, grants policy entitlement, signs a short lease, and navigates back with a state-bound callback. The listener accepts only the expected path/state, then closes.

The desktop stores only the signed lease and verifies it offline against a public-key trust set generated into the installer by `create-trusted-keyset.mjs`. It always binds verification to the locally stored device ID and uses the signed subject for achievements/sessions. Caller-supplied keys, device IDs, account IDs, booleans, and timestamps are not trust inputs.

The current lease supports offline launch until expiry. Server revocation is enforced when a refreshed lease or revocation-aware control-plane response is obtained; a client that is genuinely offline cannot learn a new server-side revocation before expiry. Suspended users receive no new lease. Rotation requires a new SRE build/trust set (or a future signed trust-set mechanism) containing the new public key before FTEP begins signing with it.
