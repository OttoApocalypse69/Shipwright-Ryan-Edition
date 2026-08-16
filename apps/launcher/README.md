# SRE desktop launcher

The Tauri 2 / React launcher provides five-step onboarding, local random device identity, Accord acceptance, catalog library, OoT import, external Wii/Wii U/Switch setup, signed entitlement enforcement, process-backed sessions, offline achievements, a separate overlay window, and sanitized diagnostics.

## UI development

From the repository root:

```powershell
pnpm install --frozen-lockfile
pnpm --filter @sre/launcher test
pnpm --filter @sre/launcher lint
pnpm --filter @sre/launcher dev:web
```

Browser preview uses fallback catalog/device/diagnostic data and cannot register files, cache a signed lease, or launch a runtime. Those actions require the Tauri desktop process.

## Native resources and desktop development

Use a Visual Studio x64 developer shell with Windows SDK, CMake, Ninja, Node/pnpm, and Rust. Build the preserved Shipwright extraction/runtime targets, then prepare the launcher resources:

```powershell
cmake -S . -B build-sre-tools -G Ninja -DCMAKE_BUILD_TYPE=Release -DSOH_TOOLS_ONLY=ON
cmake --build build-sre-tools --target soh-torch
cmake -S . -B build-sre-runtime -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build-sre-runtime --target GenerateSohOtr soh
./apps/launcher/scripts/prepare-resources.ps1 -ToolsBuild build-sre-tools -RuntimeBuild build-sre-runtime -RuntimeOutput build-sre-runtime
```

For an entitlement-capable build, generate the public trust resource from the deployment's protected Ed25519 private key. The script writes only the derived public key:

```powershell
$env:FTEP_LEASE_SIGNING_PRIVATE_KEY = '<base64 PKCS8 DER from secret store>'
$env:FTEP_SIGNING_KEY_ID = 'production-2026-01'
node tooling/scripts/create-trusted-keyset.mjs apps/launcher/resources/trust/ftep-signing-keys.json
Remove-Item Env:FTEP_LEASE_SIGNING_PRIVATE_KEY
pnpm --filter @sre/launcher tauri build
```

The checked-in empty trust set is fail-closed: it permits UI/library/diagnostics development but no lease can be cached or used. Never commit a production-derived key-set change without treating it as public release-key material and reviewing rotation timing.

The NSIS configuration installs per machine under SRE, creates normal Start menu/uninstall entries, and packages runtime, extractor, and trust resources. End users must never run these developer commands.

## One-click local startup

For this checkout's local FTEP development mode, build the small helper once:

```powershell
pnpm --filter @sre/launcher build:local-launcher
```

This puts **`START SRE.exe`** at the top level of this project. Double-click
it to open no terminal and start the same development SRE session as
`pnpm --filter @sre/launcher dev`, including
the dynamically ported local FTEP service. It is intentionally a source-checkout
helper, not an end-user release installer; it requires the already-installed
Node.js, pnpm, Docker/PostgreSQL, and local FTEP configuration. Startup output
is written to `target\sre-local-launcher.log` if troubleshooting is needed.
Repeated clicks are safe: the helper holds an OS-level instance lock and exits
successfully when SRE is already running. If a forced close left only the Vite
frontend behind, the next click reuses that frontend and starts a fresh SRE
window instead of showing a port-in-use error.

The local FTEP development server starts in the background after the native
window opens. SRE shows an indeterminate startup bar while it reads local
state, and reports a retryable timeout instead of leaving a blank window if a
native startup call does not respond.

## Generated-data cleanup

The checkout contains development build output, not just the launcher. Rust,
CMake, and verification builds can leave duplicate debug symbols and libraries
behind, while Ryujinx creates a text log for each launch. Preview reclaimable
data with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\clean-generated-data.ps1
```

The script is preview-only unless `-Apply` is supplied. It removes only
rebuildable verification/CMake output and stale Ryujinx logs by default. Add
`-IncludeRustTarget` only when you are ready to rebuild the development
launcher; add `-RemoveNodeModules` only if you are also willing to run
`pnpm install --frozen-lockfile` again. ROMs, saves, keys, firmware, source,
and bundled runtime resources are never targets. The launcher also prunes old
Ryujinx logs automatically, retaining recent diagnostics and a small bounded
history so a stuck game cannot fill the disk.

## Emulator control

Open **Emulators** from the launcher sidebar to see the managed Shipwright,
2 Ship 2 Harkinian, Cemu, Ryujinx Canary, and user-installed Dolphin runtimes. The view includes
separate buttons for opening an emulator without a game, opening its settings
folder, and opening its runtime folder so updates can be applied without
repeating game setup. Missing runtimes remain visible with their settings
folder available, while launch and runtime-folder actions stay disabled until
the executable is present.
