# SRE

## Super Runtime Environment

SRE is a Windows-first desktop game library and runtime orchestrator. It gives users a guided, catalog-driven setup for supported native ports and user-installed external runtimes while keeping proprietary game data local.

FTEP—the FICSIT Treaty Enforcement Platform—is the optional web and policy control plane. It provides browser OAuth/OIDC, random-device registration, signed offline entitlement leases, Accord policy, compatibility administration, achievements, sessions, release discovery, and audit records. Private signing material stays server-side; SRE ships only an Ed25519 trust set.

## Release-candidate scope

- Six catalog titles: Ocarina of Time, Majora's Mask, Breath of the Wild, Tears of the Kingdom, Echoes of Wisdom, and Animal Crossing: New Horizons.
- Shipwright is the supported native path for OoT. Cemu-compatible Wii U and generic Switch adapters are external/manual integrations and are labelled experimental or investigating according to evidence.
- Five-screen first run, catalog library, per-game setup, process-backed sessions, exact/approximate playable events, ten offline-first achievements, a separate click-through overlay, SRE Doctor, signed leases, and a signed updater model.
- Next.js FTEP site with user pages, a role-gated admin surface, Auth.js provider architecture, PostgreSQL migrations, parameterized API queries, CSP nonces, and GitHub release discovery.
- Per-machine NSIS installer, portable package recipe, signed update manifest, checksums, CI, and release workflow.

SRE does not distribute or locate Nintendo game data, console keys, firmware, copyrighted assets, or third-party emulator binaries. Users provide lawful game data and install external runtimes themselves. Revocation can refuse future FTEP-authorized launches; it never deletes local files, saves, runtimes, or data.

## Repository map

```text
apps/launcher/              SRE Tauri/React desktop application
apps/web/                   FTEP Next.js web and API application
crates/sre-*/               local library, device, entitlement, achievements,
                             overlay, diagnostics, and updater services
crates/ftep-core/           package sre-core: generic catalog identities/model
crates/ftep-runtime/        package sre-runtime: provider/session contract
crates/ftep-policy/         Accord and compliance policy
integrations/               native, Wii U, and generic Switch adapters
packages/game-catalog/      versioned catalog and JSON schema
packages/treaty/            canonical Accord documents
packages/third-party/       evidence-based component/distribution registry
database/migrations/        PostgreSQL control-plane schema
tooling/scripts/            validation, trust-set, packaging, and signing tools
```

## Developer gates

Install Node.js 24, pnpm 11.19, the stable Rust toolchain, and the Windows C++/SDK prerequisites used by upstream Shipwright. From the root:

```powershell
pnpm install --frozen-lockfile
pnpm check
```

Focused development:

```powershell
pnpm --filter @sre/launcher dev:web
pnpm --filter @ftep/web dev
cargo run -p sre-diagnostics --bin sre -- doctor --json
```

Native resource preparation and installer commands are in [apps/launcher/README.md](apps/launcher/README.md). Deployment and secret requirements are in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md). The [Ryan Operations Manual](docs/RYAN_OPERATIONS_MANUAL.md) is the shortest operator path.

## Current external blockers

No public release is claimed by this repository state. Production deployment requires OAuth application credentials, PostgreSQL, lease and release Ed25519 signing keys, a GitHub release target, and final third-party redistribution review. The inherited Shipwright-derived root has no confirmed top-level redistribution grant, so bundled public redistribution remains blocked pending legal evidence. External runtimes and game data are user-managed.
