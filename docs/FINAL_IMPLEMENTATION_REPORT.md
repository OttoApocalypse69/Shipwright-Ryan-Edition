# SRE / FTEP final implementation report

Date: 2026-08-13

## What changed

The repository is now a provider-oriented SRE/FTEP monorepo. SRE owns the Windows desktop experience, local data, runtime orchestration, signed-lease verification, sessions, achievements, overlay, diagnostics, and updates. FTEP owns browser authentication, device registration, Accord/policy records, entitlement issuance, compatibility administration, release discovery, audit records, and server-only signing.

The catalog contains the seven current priority titles. Runtime providers implement one generic contract. Shipwright and the bundled Two Ship runtime provide supported native paths for OoT and Majora's Mask; the Wii U adapter accepts the managed Cemu-compatible runtime; the preferred Skyward Sword HD path uses the managed Ryujinx Canary Switch adapter, with the original Wii/Dolphin path retained as a secondary variant; one generic Switch adapter serves the Switch catalog without duplicated emulator subsystems. No adapter downloads proprietary game content or external runtimes.

The launcher now has the eight required first-run screens, browser account connection, random local identity, canonical Accord ratification, catalog/runtime/system checks, searchable/filterable library status, per-game setup, signed authorized launches, process-backed sessions, playable detection, ten offline-first achievements, a separate click-through overlay, and sanitized SRE Doctor export.

The Next.js FTEP application provides public, account, compatibility, download, status, security/privacy, and role-gated admin routes plus Auth.js OAuth/OIDC provider configuration and PostgreSQL APIs. The `/connect` flow registers the random device public key, ratifies the exact Accord hash, grants policy entitlement, issues an Ed25519 lease, and returns it through a state-bound `127.0.0.1` callback. PostgreSQL migration `0001_ftep_core.sql` creates all required domain tables, constraints, and indexes.

Release tooling builds per-machine NSIS, portable ZIP, SHA-256 checksums, a release manifest, and an Ed25519 signature. The production workflow derives an installer trust set containing only the lease public key, builds/tests, packages, signs, uploads, and optionally publishes on explicit dispatch or `sre-v*` tag.

## Compatibility and runtime status

| Game | Runtime path | Status |
|---|---|---|
| Ocarina of Time | Shipwright native | Supported |
| Majora's Mask | bundled Two Ship native runtime | Supported |
| Skyward Sword HD | managed Ryujinx Canary Switch adapter | Supported |
| Breath of the Wild | managed Cemu-compatible Wii U adapter | Supported |
| Tears of the Kingdom | managed Ryujinx Canary Switch adapter | Supported |
| Echoes of Wisdom | managed Ryujinx Canary Switch adapter | Supported |
| Animal Crossing: New Horizons | managed Ryujinx Canary Switch adapter | Supported |

`Supported` applies to one exact game/variant/runtime combination. Entitlement eligibility never changes this technical label.

## Verification evidence

- `pnpm install --frozen-lockfile`: passed; all five workspace projects and supply-chain lock policy.
- `pnpm lint`: passed; TypeScript, ESLint, Rust formatting, and `cargo clippy --all-targets -D warnings`.
- `pnpm test`: passed; 52 tests total (12 TypeScript plus 40 Rust), metadata validation, synthetic runtime failures, lease states, policy transitions, achievements, overlay timing, adapters, updater signature/hash/host checks, diagnostics redaction, and catalog contracts.
- `pnpm build`: passed; 39 Next.js pages/API outputs and the complete Rust workspace.
- `cargo check --workspace --locked`: passed.
- `pnpm audit --prod`: zero known vulnerabilities.
- `cargo audit`: zero vulnerabilities after updating `time`, `plist`, and `quick-xml`. It reports 17 non-failing warnings in the Tauri cross-platform GTK3/proc-macro/Unicode dependency closure; GTK3 is not part of the Windows runtime path, but dependency maintenance remains tracked.
- Browser verification: `/`, `/compatibility`, `/games/zelda-totk`, `/dashboard`, and invalid-request `/connect` all returned 200 with no console/page errors. Anonymous `/admin` redirected to `/dashboard`. The complete eight-screen launcher flow opened a seven-card library; search reduced Animal Crossing to one card; status, filters, diagnostics, and layout passed visual inspection.
- Installer lifecycle: exact final installer silently installed to `C:\Program Files\SRE`, created Desktop and Start Menu shortcuts, launched, silently uninstalled with exit code 0, and removed installation and shortcuts.
- Package verification: final local installer and portable ZIP checksums matched; the portable archive contains SRE, runtime metadata/resources, extractor, and trust set.

## Build artifacts

Local verification artifacts (ignored by Git):

- `target/release/bundle/nsis/SRE_0.1.0_x64-setup.exe` — 16,172,736 bytes.
- `target/release/sre-launcher.exe` — 8,317,440 bytes.
- `release-assets/SRE-Setup-x64.exe` — SHA-256 `7d3cfa1eabfbbc2191e2edba8d70a0bf388b21b2d50d3ac3d7f8b0941ba96597`.
- `release-assets/SRE-Portable-x64.zip` — SHA-256 `cfbb992d10514b1fec2092b23728b52fbb0bea0c8180b6a20ea89fbc8f16a7d6`.

These are explicitly unsigned local validation artifacts. No manifest/signature was fabricated, and they must not be published. The checked-in trust set is empty and therefore fail-closed; a production workflow must derive the public trust set from the protected lease key before building.

## Security review result

Manual and automated review covered OAuth/state, cookies/session limits, CSRF/XSS/CSP, admin/API authorization, parameterized SQL, signing-key boundaries, lease device/time/signature validation, device ownership conflicts, updater signature/hash/host allowlists, diagnostic redaction, direct process argument construction, archive/resource paths, and Tauri capabilities. Important fixes made during review include per-request CSP nonces, installer-pinned lease trust, removal of boolean/account/device caller trust, ownership-safe device upsert, exact callback origin/state validation, direct process APIs, and dependency advisory upgrades.

The mandatory Codex Security workbench scan did not produce a report. Its single permitted start attempt failed before creating authoritative scan context with: `Could not read untracked file: apps/web/.next/dev/lock`. The generated lock and servers were removed afterward, but the security workflow forbids replacing or retrying a failed desktop scan in the same response. An independent workbench scan therefore remains a release gate and must be started in a fresh task against the clean, stopped repository.

## Deployment requirements supplied by Teri

- Vercel project credentials/configuration and production `FTEP_WEB_URL`.
- At least one OAuth/OIDC application with exact deployed Auth.js callback URLs.
- Production PostgreSQL connection, TLS, backup/restore, and least-privilege roles.
- Protected Ed25519 lease-signing and release-manifest-signing private keys plus key ID.
- GitHub repository variables/secrets, environments, Actions permissions, release target, and branch/tag protections.
- Final third-party redistribution/legal determination for the inherited Shipwright-derived root and complete notices.

## Known limitations

- No production Vercel/PostgreSQL/OAuth deployment was possible without external credentials; the migration is CI-defined but no local Postgres daemon was available for an additional live apply.
- The normal browser callback code is implemented, but a production end-to-end OAuth/lease test requires real OAuth, PostgreSQL, deployment origin, and production-derived launcher trust set.
- BOTW and Switch paths require user-installed external runtimes and lawful user-provided material. Compatibility remains experimental/investigating as labelled.
- A newly issued server revocation cannot reach a genuinely offline client until reconnect or cached lease expiry. Suspension prevents new leases and remains non-destructive.
- Public binary redistribution is blocked pending license evidence. Local artifacts are unsigned and not releases.
- The independent Codex Security workbench report remains incomplete due to the documented start failure.

## Recommended first deployment steps

1. Start a fresh independent Codex Security scan with all dev servers stopped; remediate every validated finding and rerun exact-head gates.
2. Complete third-party license/notice review and obtain redistribution clearance.
3. Provision PostgreSQL, apply the migration from empty, test backup/restore, and configure least privilege.
4. Register OAuth/OIDC callbacks, configure Vercel secrets, deploy FTEP preview, and verify login/admin/device/ratification/lease paths.
5. Generate protected Ed25519 keys, publish the public lease key through the installer trust-set step, and exercise rotation/revocation.
6. Run the release workflow without publication, repeat installer/upgrade/uninstall and signed-update verification, then approve an explicit stable GitHub Release.
