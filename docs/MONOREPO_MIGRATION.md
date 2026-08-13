# FTEP monorepo migration assessment

Status: approved implementation plan, based on the repository at
`14d9d4e8bb50dbcff65e2544305fb253bd9cdb72` plus the uncommitted Milestone 1.5
launcher work inspected on 2026-08-13.

This document is the Phase A audit and the migration contract for turning this
Shipwright-derived repository into the FICSIT Treaty Enforcement Platform
monorepo. It records the current facts before structural changes begin. The
first extraction deliberately leaves the upstream C/C++ tree in place and
buildable.

## Executive assessment

FTEP is not yet the architectural root of the repository. The new launcher is
an FTEP application, but its Rust backend directly knows Shipwright executable
names, asset archive names, extractor locations, version metadata, supported
ROM hashes, and source-tree paths. The React UI also presents a single-title
onboarding flow and uses Shipwright-specific copy in several places.

The safest first boundary is therefore additive:

1. preserve the verified launcher and upstream build as the behavioral baseline;
2. add root Cargo and pnpm workspaces without moving the C/C++ tree;
3. introduce stable FTEP game/catalog and runtime-provider contracts;
4. validate those contracts with a data-driven catalog and synthetic providers;
5. move the existing OoT detection/import/launch behavior behind a
   `ShipwrightAdapter` only after the contracts are tested.

Moving `soh/`, `torch/`, or `libultraship/` during the first extraction would
combine source relocation, submodule changes, CMake path changes, packaging
changes, and adapter work in one step. That is a high-risk, low-value big-bang
migration and is explicitly deferred.

## Current repository inventory

### Ownership map

| Path | Current role | Ownership and migration treatment |
| --- | --- | --- |
| `apps/launcher/` | Tauri 2 + React 19 Windows-first FTEP launcher | FTEP-owned. Preserve and make it a Cargo/pnpm workspace member. |
| `packages/treaty/` | Versioned machine-readable treaty | FTEP-owned. Keep published treaty bytes append-only. |
| `docs/FTEP_SPECIFICATION.md` | Product and Milestone 1.5 specification | FTEP-owned. Preserve. |
| `docs/supportedHashes.json` | Shipwright-supported OoT source hashes | Upstream-derived integration input. It belongs behind the Shipwright adapter, not in FTEP core. |
| `soh/` | Ship of Harkinian application/game code and assets | Upstream Shipwright. Do not make FTEP domain crates depend on it. |
| `torch/` | Maintained asset extraction tooling | Git submodule at `e92c210...`; upstream integration dependency. |
| `libultraship/` | Runtime/framework dependency | Git submodule at `9a1a2bd...`; upstream integration dependency. |
| root `CMakeLists.txt` and `CMake/` | Shipwright build and packaging root | Keep operational while the native runtime remains in-tree. |
| `.github/workflows/` | Shipwright C/C++ formatting and multi-platform build/release workflows | Upstream-oriented. Add FTEP gates separately before replacing any workflow. |
| `roms/` | Empty/placeholder local input structure used by upstream tooling | Never populate or distribute proprietary game data. |

The inspected working tree contains only two direct modifications to upstream
build files:

- root `CMakeLists.txt` aligns the MSVC runtime for the `soh-torch` and
  `soh-o2r-packer` entry points with their static dependencies;
- `soh/CMakeLists.txt` disables an unnecessary `dr_libs` submodule checkout.

Both are thin, build-oriented integration patches. No inspected FTEP change
modifies gameplay code, saves, proprietary assets, or upstream engine behavior.

### Existing FTEP implementation

The Milestone 1.5 launcher currently provides:

- a resumable thirteen-stage React onboarding flow;
- canonical treaty display and local acceptance metadata;
- local streaming SHA-1 validation against Shipwright's supported OoT catalog;
- a cancellable Shipwright/Torch import process using application-managed
  staging and atomic promotion;
- atomic launcher state persistence;
- native Windows Direct3D 11 and XInput probes;
- friendly preflight diagnostics;
- protocol-gated discovery and launch of the bundled Shipwright runtime;
- a clearly non-playable synthetic onboarding path;
- NSIS per-machine packaging and uninstall support.

The synthetic path and native launch gate correctly prevent synthetic data from
starting a game. The importer does not alter the selected source file, and a
failed/cancelled import is not promoted.

### Coupling that must be removed incrementally

| Coupling | Location today | Target owner |
| --- | --- | --- |
| `soh.exe`, `soh.o2r`, `ftep-runtime.json` discovery | launcher `runtime.rs` | `integrations/native/shipwright` |
| Shipwright protocol/version parsing | launcher `runtime.rs` and resource JSON | Shipwright adapter + versioned FTEP protocol |
| `soh-torch.exe` and `soh/assets/yml` lookup | launcher `importer.rs` | Shipwright adapter |
| OoT archive names and extraction version | launcher `importer.rs` | Shipwright adapter manifest |
| Shipwright supported ROM hashes and N64 byte-order validation | launcher `lib.rs` | Shipwright source-validation adapter |
| `launch_zelda` single-product command | launcher Rust/TypeScript bridge | game/session application service using `GameId` and `RuntimeId` |
| single-title onboarding state | React domain and `App.tsx` | reusable per-game setup state + library state |
| `shipwrightVersion` in generic launch response | Rust/TypeScript bridge | generic runtime identity/version fields |
| source-tree-relative paths | launcher Rust modules and package resources | adapter configuration supplied by the application shell |

FTEP core must never acquire a dependency on any item in the left-hand column.
The dependency must point from the Shipwright adapter to FTEP interfaces.

### Implemented versus specified

| Capability | Current state |
| --- | --- |
| Tauri/React desktop launcher | Implemented and locally verified. |
| OoT source validation/import/Shipwright launch | Implemented, but directly coupled. |
| Treaty document | Implemented as a versioned JSON package. |
| Authentication | Local onboarding placeholder only; no OAuth/OIDC exchange exists. |
| Device identity | Local onboarding placeholder only; no keypair or registration exists. |
| Entitlements | Local boolean launch gate only; no signed lease, revocation, or product scope exists. |
| PostgreSQL/control plane | Not implemented. |
| Web application | Not implemented. |
| Game library/catalog | Not implemented before this migration. |
| Runtime registry/provider interface | Not implemented before this migration. |
| Sessions/runtime events/local IPC | Not implemented. |
| Achievements and overlay | Not implemented. |
| Signed desktop updater | Not implemented. |
| Compatibility/status administration | Not implemented. |

The local placeholders are useful UI seams, not production security controls.
They must remain visibly labelled and must not be presented as cryptographic
authorization.

## Tests and release behavior protecting the baseline

Current FTEP checks:

- `npm.cmd run check` in `apps/launcher`: four Vitest onboarding-state tests,
  TypeScript compilation, and a Vite production build;
- `cargo test --manifest-path apps/launcher/src-tauri/Cargo.toml`: nine Rust
  tests covering supported hashes, N64 byte order, treaty identity, atomic
  writes/copies, runtime metadata rejection, launch gates, repository resource
  discovery, and native probe result quality;
- `cargo clippy --manifest-path apps/launcher/src-tauri/Cargo.toml --all-targets -- -D warnings`;
- the Shipwright CMake build and generated runtime/extractor artifacts;
- Tauri NSIS packaging plus manual install/uninstall lifecycle verification.

Existing GitHub workflows build and format Shipwright on supported desktop
platforms. They do not run the new Rust or TypeScript checks. A monorepo CI job
must be added before the root workspace becomes a release gate; existing
Shipwright workflows stay in place until the replacement proves equivalent.

The current installer bundles FTEP-owned launcher code plus an in-tree
Shipwright runtime, Torch extractor, generated resource archive, and extraction
definitions. There is no signed update client. GitHub Releases remains the
intended binary store, but signed manifests are future work.

## Licensing and distribution boundaries

The repository root has no top-level `LICENSE` file. This is a release blocker
for making new assumptions about the license of the Shipwright-derived root;
absence of a license is not permission to redistribute. Before a public FTEP
binary or source release, maintainers must confirm the applicable upstream
Shipwright terms and document the license for every included source and binary.

Known local evidence:

- `libultraship/LICENSE`: MIT;
- `torch/LICENSE`: MIT;
- `torch/lib/StormLib/LICENSE`: MIT;
- `soh/soh/Enhancements/randomizer/3drando/LICENSE.md`: attribution for
  MIT-licensed derivative code;
- CMake/vcpkg dependencies and fetched `spdlog`/`dr_libs` introduce additional
  notices that are not yet collected by FTEP;
- npm and Cargo dependencies require generated acknowledgements before public
  distribution.

Each runtime/integration will have a registry record containing project URL,
license, integration type, version, redistribution mode, attribution
requirements, and source availability. Unknown values block bundling. An
adapter may still support an `EXTERNAL` or `MANUAL_ONLY` runtime without FTEP
redistributing it.

FTEP will never contain or retrieve Nintendo ROMs, game images, firmware,
console keys, title keys, authentication secrets, or proprietary assets. Game
data and any legitimately required console-derived material remain
user-supplied. Runtime integration must not implement circumvention or search
piracy sources.

## Target monorepo

The target is reached incrementally; directories appear only when they contain
working code or an immediately useful manifest.

```text
ftep/
|-- apps/
|   |-- launcher/                  # React shell + Tauri composition root
|   `-- web/                       # later: Next.js/Vercel UI
|-- services/
|   `-- control-plane/             # later: domain API, no provider-specific DB calls
|-- packages/
|   |-- game-catalog/              # versioned catalog data + JSON Schema
|   |-- protocol/                  # later: cross-language versioned schemas
|   |-- treaty/                    # existing immutable treaty documents
|   |-- ui/                        # later: shared web/launcher UI primitives
|   `-- third-party/               # component registry + generated notices
|-- crates/
|   |-- ftep-core/                 # identifiers and provider-neutral domain values
|   |-- ftep-library/              # later: installs, preference, runtime resolution
|   |-- ftep-runtime/              # provider interface, registry, events, sessions
|   |-- ftep-entitlement/          # later: signed product-scoped leases
|   |-- ftep-device/               # later: random UUID + local keypair
|   |-- ftep-achievements/         # later: local-first rule engine
|   |-- ftep-overlay/              # later: reusable non-invasive overlay
|   |-- ftep-diagnostics/          # later: structured/sanitized support data
|   `-- ftep-updater/              # later: signed manifest verifier
|-- integrations/
|   |-- native/
|   |   |-- shipwright/            # first real adapter
|   |   |-- two-ship/              # slot only when implementation begins
|   |   `-- zelda3/                # slot only when implementation begins
|   `-- emulator/
|       |-- cemu/                  # later: BOTW external adapter
|       `-- switch/                # later: generic Switch interface
|-- runtimes/manifests/            # acquisition/version/distribution policy
|-- database/{migrations,seeds}/   # later: portable PostgreSQL migrations
|-- tooling/{scripts,release}/
|-- docs/
|-- soh/                           # preserved upstream source until separately extracted
|-- torch/                         # preserved pinned submodule
|-- libultraship/                  # preserved pinned submodule
|-- Cargo.toml
|-- package.json
`-- pnpm-workspace.yaml
```

Turborepo is deferred while there is only one JavaScript application. pnpm and
Cargo already provide the required task fan-out; adding a cache/orchestration
layer now would not solve a current problem. It can be introduced when `apps/web`
or `services/control-plane` creates a real multi-package task graph.

## Dependency rules

Allowed direction:

```text
React/Tauri composition root
        -> application/domain crates
        -> ftep-core + ftep-runtime interfaces
        <- runtime adapters
```

Enforced rules:

- `ftep-core` is provider-neutral and cannot depend on Tauri, Shipwright,
  emulator APIs, filesystem layout, or platform process APIs.
- `ftep-runtime` depends on `ftep-core`, not on any concrete adapter.
- adapters depend on the stable interfaces and may depend on upstream/runtime
  details.
- UI code calls application commands and consumes catalog/protocol data; it
  does not branch on executable names or infer identity from filenames.
- treaty policy may be Zelda-specific, but identifiers, library, runtime,
  session, event, and achievement infrastructure remain franchise-neutral.
- database access is behind a service/package boundary and uses standard
  PostgreSQL migrations.
- control-plane responses cannot instruct the client to execute arbitrary
  commands or unsigned URLs.

## Migration phases and gates

### Phase A - audit and foundations

Deliver this assessment, root workspaces, small root commands, a third-party
registry seed, and CI-ready local checks. No source relocation.

Gate: current frontend/Rust/Clippy checks and the existing Shipwright build
continue to pass.

### Phase B - provider-neutral core

Add stable validated `GameId`, `RuntimeId`, game/variant/source models, a
versioned data-driven catalog, runtime types/capabilities, a provider trait,
structured errors/events, and a duplicate-safe provider registry. Add
synthetic providers for deterministic discovery/resolution/lifecycle tests.

Gate: `cargo test --workspace`, strict workspace Clippy, catalog schema/semantic
validation, and root pnpm checks.

### Phase C - Shipwright adapter

Create `integrations/native/shipwright`. Move executable/resource detection,
metadata validation, source validation, Torch preparation, verification,
configuration, and launch out of the Tauri composition root in small slices.
Preserve cancellation, atomic promotion, diagnostics, and the real OoT path at
every slice. Change generic bridge responses from `shipwrightVersion` to
`runtimeId`/`runtimeVersion` only with a versioned state/IPC migration.

Gate: adapter contract tests plus the existing real-data preparation and launch
preflight. The installed OoT flow must remain operational.

### Phase D - provider-independent launcher proof

Wire a non-playable synthetic provider through the same registry, library,
setup, diagnostics, and session surfaces. Replace scattered game-specific UI
branches with catalog-driven rendering.

Gate: launcher completes the fixture flow with Shipwright absent; synthetic
data is still rejected by all real launch paths.

### Phase E - library and second native slot

Introduce installations, explicit runtime preferences, resolver health, and
session persistence. Add Majora's Mask/2Ship only after licensing, source
workflow, and distribution records are known.

### Phase F - external runtime foundation

Implement external executable discovery/selection, version policy, process and
window lifecycle fallback, controller/config/save-path seams, ordinary-language
setup errors, and `EXTERNAL`/`MANUAL_ONLY` acquisition modes. Never download
game material or proprietary console material.

### Phases G-H - BOTW then generic Switch architecture

Add a Wii U adapter pathway for user-supplied BOTW data, then a generic Switch
runtime interface for TOTK. Both use the same library/session/event pipeline.
Mark compatibility honestly; architecture-only entries remain `INVESTIGATING`
or `EXPERIMENTAL`, never `SUPPORTED`.

### Phases I-J - web/control plane and release operations

Add the Vercel-targeted web app, PostgreSQL migrations, OAuth/OIDC browser
login, non-invasive device keys, signed product-scoped offline leases,
compatibility administration, public status, signed releases/updates,
diagnostics, and release channels. Achievements remain local-first so the
`Ahh, Zelda` notification never waits for a server round trip.

## Potentially destructive changes

The following require a dedicated change, an exact rollback plan, and fresh
end-to-end evidence. None belongs in the first extraction:

- moving or converting the `soh`, `torch`, or `libultraship` roots;
- rewriting submodule history or replacing pinned upstream commits;
- deleting the nested launcher lockfiles before root workspace lockfiles are
  proven on clean machines;
- changing app-data paths or onboarding-state schema without migration;
- renaming Tauri commands without a coordinated TypeScript bridge update;
- changing installer identity, scope, upgrade code, or resource layout;
- replacing existing Shipwright CI/release workflows before equivalent gates
  run successfully;
- changing published treaty bytes;
- relabeling a runtime pathway `SUPPORTED` without compatibility evidence;
- bundling a runtime whose license/redistribution status is unknown;
- any operation that touches user game data, imported archives, or saves.

FTEP revocation will only refuse future FTEP-authorized launches. It will never
delete, corrupt, encrypt, or modify game data, saves, or third-party runtimes.

## Immediate implementation slice

The smallest coherent extraction following this assessment is:

1. add root Cargo and pnpm workspaces while keeping existing commands working;
2. add `ftep-core` and `ftep-runtime` crates;
3. add a versioned catalog for OoT, MM, BOTW, and TOTK with honest current
   compatibility states;
4. add runtime types, capabilities, events, provider registry, and synthetic
   contract tests;
5. seed the third-party component registry with known facts and explicit
   `UNKNOWN` release blockers;
6. verify the workspace and unchanged launcher gates;
7. only then start the separately testable `ShipwrightAdapter` migration.

This slice makes FTEP the owner of stable domain contracts without pretending
the current launcher is already decoupled. It is additive, reversible, and
keeps the real OoT implementation available throughout the next phase.

## Implementation progress

The first implementation pass completed Phase A, Phase B, and one bounded
Phase C slice:

- root Cargo/pnpm workspaces and root developer commands exist;
- `ftep-core`, the versioned catalog, and semantic catalog tests exist;
- `ftep-runtime`, the provider registry, runtime events, and synthetic provider
  contract tests exist;
- runtime manifests and the third-party release-blocker registry exist;
- `integrations/native/shipwright` now owns runtime metadata validation,
  discovery, imported-installation verification, atomic `soh.o2r` staging, and
  process launch;
- the Tauri bridge now sends an explicit `GameId` and returns generic runtime
  identity/version fields.

Shipwright source hashing, Torch discovery/preparation, cancellation, and the
imported-asset manifest remain in the launcher and are the next Phase C slice.
This is an explicit seam, not a claim that Phase C is complete.
