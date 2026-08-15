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

This is the implementation ledger. Update it with every material migration
change and record verification evidence before treating a phase as complete.

### Phase A — completed

- Root Cargo/pnpm workspaces, small root commands, metadata validation, and
  the third-party evidence registry are in place.
- The original Shipwright source/build roots remain in place; no upstream
  relocation or history rewriting occurred.

### Phase B — completed

- `sre-core` supplies validated IDs, catalog models, and semantic catalog
  checks.
- `sre-runtime` supplies provider contracts, registry resolution, structured
  errors/events, and deterministic synthetic-provider tests.

### Phase C — completed

- `integrations/native/shipwright` owns Shipwright executable/resource
  discovery, metadata/version validation, supported OoT source hashes, N64
  byte-order recognition, Torch discovery/preparation/cancellation, imported
  asset manifests, verification, atomic resource staging, and guarded launch.
- The Tauri launcher supplies only app/resource roots and UI command plumbing;
  Shipwright filenames, hashes, archive names, and extractor paths no longer
  appear in the launcher composition root.
- The durable import operation remains a cancellable adapter API rather than
  the generic `RuntimeProvider::prepare` method because it must atomically
  promote a desktop manifest.

### Phase D — completed

- The catalog-backed library, synthetic runtime fixtures, diagnostics, and
  session model share provider-neutral contracts.
- Synthetic fixtures remain rejected by every authorized launch path.

### Phase E — completed for the supported path

- Installations, session persistence, explicit runtime selection, and resolver
  health are implemented.
- Native OoT import now creates or updates the ready Shipwright library
  installation, so the documented **Set up → Play** path reaches the same
  provider/session service as external runtimes. The obsolete single-title
  launcher command was removed.
- A Link to the Past remains excluded from the active catalog until independent
  source, adapter, and redistribution evidence exists.

### Phases F–H — completed as external/manual integrations

- The Cemu-compatible Wii U adapter and generic Switch adapter validate
  explicit user-selected sources/runtimes, launch them with direct argument
  arrays, persist sessions, and label playable detection as approximate.
- SRE never downloads runtimes, console material, firmware, keys, or game
  data. Compatibility is limited to the evidence-based labels in the catalog.

### Phase I — completed in local source; deployment is operator-gated

- The Next.js FTEP application, Auth.js configuration, PostgreSQL migration,
  device registration, Accord ratification, signed lease issuance, role-gated
  compatibility administration, status/download pages, and audit writes are
  implemented and covered by local tests.
- A live sign-in → device → ratification → lease round trip still requires an
  actual PostgreSQL deployment, OAuth/OIDC client, HTTPS origin, and protected
  Ed25519 signing key. No placeholder credentials or fabricated success path
  may substitute for them.

### Phase J — completed in local source; publication is operator-gated

- Signed release-manifest verification, installer/portable packaging scripts,
  release workflow, updater restrictions, diagnostics export, channels, and
  GitHub Release discovery are implemented.
- Public publication remains prohibited until the third-party registry no
  longer contains blocked redistribution evidence and protected production
  release/lease keys plus repository release permissions are configured.

### Verification status — 2026-08-14

- `cargo test --workspace --locked`: passed after loading the Windows x64 MSVC
  developer environment.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `pnpm --filter @sre/launcher check`: passed (four frontend tests plus
  TypeScript/Vite production build).
- `pnpm check`: passed end-to-end (workspace lint, Rust formatting and strict
  Clippy, launcher/web tests, metadata validation, the full Rust test suite,
  Next.js production build, and workspace Cargo build).
- `pnpm audit --prod --audit-level high`: passed with no known
  vulnerabilities.
- `cargo-audit` v0.22.2 was installed locally and `cargo audit` completed
  without vulnerability advisories. It reports 17 non-failing maintenance
  warnings in the Tauri cross-platform GTK/proc-macro dependency closure;
  these remain dependency-maintenance work rather than a Windows release
  vulnerability claim.
- Native Shipwright setup: the Windows x64 extractor (`soh-torch.exe`),
  runtime (`soh.exe`), and `soh.o2r` resource archive were built from the
  preserved source tree and staged into the launcher resources. The Tauri
  development launcher then opened as a responsive `SRE — Super Runtime
  Environment` desktop window; SRE Doctor recognized the selected runtime.
- `database/migrations/0001_ftep_core.sql`: applied successfully to a
  disposable PostgreSQL 17 instance with `ON_ERROR_STOP=1`; all 27 public
  tables were created, including the device, lease, session, compatibility,
  and audit tables. The container was removed after verification.
- `sre doctor --json`: passed and emitted a sanitized local report.
- Browser verification against the local FTEP dev server: `/`,
  `/compatibility`, invalid `/connect`, and anonymous `/admin` passed. The
  latter redirected to `/dashboard`; the invalid connection request was
  rejected before OAuth/database access. `127.0.0.1` was added to
  `allowedDevOrigins` after the check exposed development-only HMR warnings.

### Local FTEP account and lease remediation — 2026-08-14

- `database/migrations/0002_local_credentials.sql` adds the nullable
  `users.password_hash` column needed only for explicit development-only local
  accounts; deployed FTEP remains OAuth/OIDC-only.
- Local account sign-up/sign-in now validates input, stores salted scrypt
  verifiers, uses non-enumerating sign-in failures, and returns the user to the
  state-bound SRE `/connect` request after authentication.
- A local PostgreSQL 17 service named `ftep-local-postgres` is running with
  migrations `0001` and `0002` applied. The ignored `apps/web/.env.local`
  holds its development signing key and database URL.
- The SRE trust set contains the corresponding public key marked
  `developmentOnly`; debug builds accept it and release builds reject it.
  The library sidebar now exposes **Connect / refresh FTEP** for users who
  completed first-run in offline mode.
- Live browser verification completed: sign-up persisted a scrypt verifier and
  audit record; bad-password sign-in displayed a generic error; good-password
  sign-in reached the dashboard; device registration, Accord ratification,
  active entitlement creation, and a non-empty unexpired Ed25519 lease all
  completed against the local database. The direct browser check intentionally
  used a non-listening callback port, so its final `ERR_CONNECTION_REFUSED`
  confirms only that the actual Tauri loopback listener must be initiated from
  **Connect / refresh FTEP**.
- The Tauri opener capability now scopes browser handoffs to HTTPS FTEP
  addresses plus loopback-only local development origins. A live SRE click
  progressed to **Waiting for FTEP...** and opened the system browser,
  replacing the previous blocked-local-URL failure.
- In a development build, SRE now starts its own local FTEP child process on
  an unused `127.0.0.1` port at launcher startup, exposes that exact URL to
  desktop connection/dashboard actions, health-checks it before use, and
  terminates only that child process tree when the main SRE window closes.
  Docker/PostgreSQL and all unrelated processes remain untouched; this
  prevents local FTEP from taking port 3000 or remaining behind after the
  launcher closes.
- `START SRE.exe` is a terminal-free, source-checkout helper placed at the
  project root for one-click local startup. It starts and later stops only its
  own Vite process tree; the SRE process continues to own the temporary FTEP
  child and stops it when the main window closes. Docker/PostgreSQL and
  unrelated processes remain untouched.
- OoT setup now stages Shipwright's non-ROM extractor assets beside
  `soh-torch.exe` and runs Torch from that directory. This fixes the missing
  `assets/`/O2R generation failure without bundling, copying, or altering a
  user's game data. Achievement overlays now display for 15 seconds and return
  to polling the queue rather than remaining visible indefinitely.
- The Shipwright runtime executable and `soh.o2r` are now staged directly in
  the imported OoT installation before launch. The executable therefore finds
  the generated `oot.o2r` beside itself instead of looking in the launcher
  resource directory and incorrectly reporting missing extractor assets.
- Shipwright's runtime-owned `assets/` directory is also staged atomically
  beside the imported archive and version-marked for reuse. This satisfies the
  runtime's mandatory first-run asset check without copying or modifying the
  user's ROM; only the generated archive and application-owned runtime files
  live in the managed installation.
- The root `START SRE.exe` helper now refuses a second startup while its
  loopback frontend is already active, preventing duplicate launcher windows
  and duplicate temporary local-service processes.
- On Windows, the managed development FTEP service is contained in a
  kill-on-close job object. A forced SRE termination now also ends its pnpm,
  Next.js, and port-owning descendants, so a future launcher startup cannot be
  blocked by a stale `.next` development lock.
- Local-game launch work, including any first-run Shipwright runtime staging,
  now runs on a blocking worker rather than the Tauri command/UI thread. The
  library keeps rendering a visible preparation state while the game starts;
  `START SRE.exe` also starts the already-built launcher binary directly
  instead of invoking Cargo at every startup.
- Shipwright now version-marks the runtime files it stages beside imported
  OoT assets and reuses a verified matching copy on later launches. First-run
  staging is cancellation-aware; the library exposes **Cancel launch** while
  preparation is in progress and **Stop game** only for a process that this
  current SRE instance started. The catalog also carries HTTPS-only remote banner
  references, rendered with a local color/title fallback rather than bundling
  third-party artwork into the repository or a user's game installation.
- The live library now polls only while SRE owns a running game process. Closing
  a game normally (including Alt+F4) is detected by the runtime monitor and the
  UI updates back from **Stop game** to **Play** within half a second.
- Every configured game card now shows its own persisted total playtime,
  calculated from completed sessions for that game rather than the library-wide
  total alone.
- `integrations/native/two-ship` now owns the managed Majora's Mask native-port
  adapter. The reviewed official 2 Ship 2 Harkinian 5.0.0 Windows bundle is
  packaged as an SRE resource, launched with the original user-selected ROM
  path, and tracked with the same owned-process/session lifecycle as OoT. SRE
  neither copies nor retains the ROM; the upstream runtime validates it and
  generates its own archive on first launch. The runtime manifest records the
  upstream project, CC0-1.0 distribution basis, version, source URL, and SHA-256
  before allowing bundled distribution.
- The managed Two Ship bundle is staged once from the packaged read-only
  resources into SRE's writable app-data runtime directory before launch. This
  satisfies Two Ship's own first-run write checks without copying the selected
  ROM, while the library keeps the original user-owned ROM path. A five-second
  UI stop-request timeout also clears a stale **Stopping...** state and refreshes
  the owned-process list instead of leaving the card disabled indefinitely.
- The library now requires an actual stop-request session ID before rendering a
  card as **Stopping...**. This prevents a newly configured game with no session
  history from comparing two absent values and having its initial **Play** button
  incorrectly disabled.
- A Link to the Past and its Zelda3 catalog candidate were removed from the
  active library, catalog contract test, and catalog documentation at the user's
  request. Existing user files are untouched; SRE no longer presents a setup
  card for that title.
- Cemu 2.6 from the official `cemu-project/Cemu` Windows release is now
  installed as a managed local runtime for Breath of the Wild. SRE records the
  release source and SHA-256, automatically selects that executable during BotW
  setup, and still requires a user-supplied Wii U game directory. It does not
  acquire or store games, console keys, or firmware; Switch runtimes remain
  manual because their required console material cannot be safely managed by
  SRE.
- The Cemu-compatible Wii U adapter now accepts either a dumped `code/*.rpx`
  installation or a user-selected `.wux` image and passes that exact path to
  Cemu. SRE does not decrypt, unpack, or modify the selected image.
- BotW setup now uses an explicit Wii U picker that displays `.wux`, `.wud`, and
  `.rpx` files, avoiding an ambiguous generic file picker that could hide the
  user's selected Wii U image.
- The Cemu-compatible Wii U adapter now accepts `.wud` disc images as well as
  `.wux` images and RPX title directories, with validation and setup guidance
  kept in sync.
- BotW setup now also accepts Cemu's single-file `.wua` archive format in both
  its picker and validation flow.
- TotK and the other Switch catalog entries now have a managed Suyu 0.0.3
  runtime. SRE stages it to the user's app-data directory and only accepts
  user-provided Switch game images; it does not provide or retrieve keys,
  firmware, or games.
- The BotW Wii U/Cemu catalog path is promoted to `SUPPORTED` after successful
  user verification with a legally supplied Wii U image and local runtime setup.
- The bundled-catalog verification now permits that verified external BotW path
  while retaining the `SUPPORTED` guard for all remaining unverified entries.
- Removed the bundled Suyu runtime. Switch entries, including TotK, now use a
  Ryujinx-ready manual executable selection until a verifiable official
  Ryujinx release channel is available for bundling.
- With the user's explicit authorization, bundled Ryujinx 1.3.3 replaces that
  manual selection. The downloaded Windows archive SHA-256 is
  `42B48CE6B3DADDED68591B1B2B7D8D727E8436A18AB6363610042FA383BE3992`.
- Replaced the elevation-requesting download with the user's supplied
  Ryubing/Ryujinx 1.3.3 archive. Its SHA-256 is
  `BAA93F48B012EEFECA339A78A39844599CCD194C109093F354A4B9D1E03A056E`.
- Existing Switch library entries now resolve the managed Ryujinx executable at
  launch time, preventing stale stored paths from continuing to start the
  retired elevation-requesting runtime.
- TotK's managed Ryujinx path is promoted to `SUPPORTED` following successful
  user verification with their supplied game, keys, and firmware.
- Echoes of Wisdom and Animal Crossing: New Horizons use the same managed
  Ryujinx Switch runtime as TotK; their setup screens therefore require only
  the user's game image, with no separate emulator executable selection.
- Echoes of Wisdom is promoted to `SUPPORTED` after user verification of its
  managed Ryujinx path.
- Local FTEP account registration and sign-in now remain usable when the
  optional development PostgreSQL service is offline. The development-only
  fallback writes salted scrypt password verifiers to an account store under
  the current user's LocalAppData directory (or the explicit
  `FTEP_LOCAL_ACCOUNT_STORE` path); it is never enabled in production.
- Development device registration now rebinds SRE's retained local device
  identity after a local account is recreated. Production registrations remain
  strict: an existing device can never be reassigned to another account there.
- All Switch catalog entries now resolve the experimental Ryujinx Canary 1.3.340
  runtime. The previous bundled stable Ryubing/Ryujinx 1.3.3 runtime was
  removed at the user's request; SRE still requires user-provided game data,
  keys, and firmware.
- Canary was upgraded after the older 1.3.31 build stalled on Animal Crossing
  v3.0.3. The verified 1.3.340 build loads the same user-supplied NSP, detects
  its update/DLC, initializes PTC, and reaches shader loading in a controlled
  smoke test.
- Managed Switch launches now pass Ryujinx's `--hide-updates` flag. Canary
  builds otherwise attempt the retired GitHub release endpoint before opening
  the game and can show a blocking update-check error even when persisted
  settings disable startup checks.
- Local development now prefers the checked-in Canary package over any stale
  `target/debug` runtime staging, and the staged development copy is refreshed
  to 1.3.340. This prevents an older 1.3.31 executable from being selected
  after rebuilding the launcher.
- The terminal-free `START SRE.exe` helper now holds an OS-level instance lock,
  treats duplicate clicks as a successful handoff, and detects older helper
  processes without showing a port-in-use dialog. After a forced close leaves
  only Vite on `127.0.0.1:1420`, the next start reuses that frontend and opens
  a fresh SRE window. A repeated-start smoke test passed with exit code 0 and
  no project child processes left behind after shutdown.
- The launcher now exposes an **Emulators** sidebar view for the managed
  Shipwright, 2 Ship 2 Harkinian, Cemu 2.6, and Ryujinx Canary runtimes. Each
  card reports availability and provides actions to open the emulator without
  a game, open its settings directory, or open its runtime directory for
  updates. Ryujinx is launched with `--hide-updates` so its retired release
  check cannot block startup, and 2 Ship is staged to a writable app-data copy
  before it is opened. The web build, lint/tests, offline Rust checks, local
  launcher rebuild, and browser smoke verification all passed.
