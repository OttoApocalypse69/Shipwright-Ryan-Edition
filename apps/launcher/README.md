# SRE desktop launcher

The Tauri 2 / React launcher provides five-step onboarding, local random device identity, Accord acceptance, catalog library, OoT import, external Wii U/Switch setup, signed entitlement enforcement, process-backed sessions, offline achievements, a separate overlay window, and sanitized diagnostics.

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
