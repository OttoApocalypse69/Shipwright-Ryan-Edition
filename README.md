# FTEP

## FICSIT Treaty Enforcement Platform

FTEP is a Windows-first game-library and runtime-orchestration platform for an
unreasonable but carefully engineered purpose: make supported Zelda pathways
pleasant to set up while administering the Great Zelda-Satisfactory Accords.

FTEP owns the user experience, library, policy, entitlements, achievements,
configuration, and orchestration. Native ports and emulators are replaceable
runtime providers. Ship of Harkinian/Shipwright is the first provider; it is not
the platform architecture.

## Current state

The verified Milestone 1.5 vertical slice includes a Tauri/React launcher,
local OoT validation and asset preparation, native Windows diagnostics, guarded
Shipwright launch, a non-playable synthetic onboarding path, and an NSIS
installer. The first monorepo extraction adds provider-neutral game catalog and
runtime contracts without moving or rewriting upstream Shipwright sources.

Not implemented yet: production OAuth/OIDC, PostgreSQL control plane, signed
device entitlements/offline leases, achievements, overlay, multi-game setup,
or signed updates. UI placeholders are not production security controls.

## Repository map

```text
apps/launcher/              FTEP desktop launcher
crates/ftep-core/           stable IDs and provider-neutral game catalog model
crates/ftep-runtime/        runtime interface, events, registry, synthetic fixtures
packages/game-catalog/      versioned data-driven catalog and schema
packages/treaty/            immutable versioned treaty documents
packages/third-party/       integration license/distribution registry
runtimes/manifests/         runtime acquisition and release policy
soh/                        preserved upstream Shipwright application source
torch/                      pinned upstream extraction-tool submodule
libultraship/               pinned upstream runtime-framework submodule
```

See [the migration assessment](docs/MONOREPO_MIGRATION.md) for the audited
current coupling, exact target tree, risk boundaries, and phased plan.

## Developer commands

Prerequisites for the workspace checks are Node.js, pnpm, and Rust:

```powershell
pnpm install
pnpm test
pnpm lint
pnpm build
```

Focused Rust commands:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`pnpm dev` starts the desktop launcher and therefore needs the Windows native
toolchain plus prepared Shipwright runtime resources. Use `pnpm dev:web` for
the launcher UI without the native shell. Detailed native build and packaging
steps live in [apps/launcher/README.md](apps/launcher/README.md); upstream CMake
requirements remain in [docs/BUILDING.md](docs/BUILDING.md).

Turborepo is intentionally not present while the repository has only one
JavaScript application. pnpm and Cargo provide the current task graph without
another orchestration layer.

## Game data and runtime policy

FTEP does not include, locate, or download Nintendo ROMs, game images,
copyrighted game assets, firmware, console keys, title keys, or authentication
secrets. Users supply their own compatible game data and any legitimately
required runtime material. FTEP does not implement circumvention logic.

Compatibility labels are evidence-based. At this stage only the preserved OoT
Shipwright pathway is marked `SUPPORTED`; MM, BOTW, and TOTK entries describe
scoped architectural targets and remain `INVESTIGATING`.

## Licensing status

This Shipwright-derived root currently has no top-level upstream license file.
Public redistribution is blocked until the applicable Shipwright terms and all
bundled dependency notices are confirmed. Known and unknown evidence is
recorded without guessing in `packages/third-party/components.v1.json`.
