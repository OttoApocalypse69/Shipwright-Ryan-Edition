# Desktop releases and updates

GitHub Releases is the binary origin. Channels are `stable`, `beta`, and `nightly`; SRE never changes channel silently. Exact public asset names are `SRE-Setup-x64.exe`, `SRE-Portable-x64.zip`, `SHA256SUMS.txt`, `release-manifest.json`, and `release-manifest.sig`.

The Windows workflow builds native resources, runs `pnpm check`, generates the public launcher trust set from the protected lease key, builds Tauri NSIS, creates the portable archive/checksums, signs the manifest with the protected release Ed25519 key, uploads artifacts, and optionally publishes. Tag-triggered releases use `sre-v*`.

The updater accepts only HTTPS GitHub URLs, allowlisted asset names, a valid Ed25519 manifest signature, matching size, and matching SHA-256. It does not trust filenames or transport alone. Key rotation requires shipping an already trusted verifier before using a new signing key.

Local unsigned builds can validate compilation and installer lifecycle with `package-sre-release.ps1 -UnsignedLocal`; that mode deliberately emits no manifest or signature and its assets must not be published. Public release is blocked until production secrets and third-party redistribution clearance exist.
