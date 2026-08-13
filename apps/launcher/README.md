# FTEP launcher

This is the Milestone 1.5 Windows-first launcher vertical slice described in [`docs/FTEP_SPECIFICATION.md`](../../docs/FTEP_SPECIFICATION.md).

Implemented now:

- Tauri 2 and React application boundary;
- resumable thirteen-stage onboarding state machine;
- canonical fourteen-article treaty viewer and acceptance metadata;
- native file picker and local streaming game-data validator;
- Shipwright supported-hash catalog integration;
- maintained Shipwright extractor adapter with cancellation and atomic promotion;
- atomic native onboarding-state persistence;
- Windows Direct3D 11 and XInput probes;
- pre-flight command and friendly expandable diagnostics with correct launch severity;
- protocol-gated Shipwright runtime discovery and safe launch through the
  provider-neutral FTEP runtime interface and `ShipwrightAdapter`;
- synthetic, explicitly non-playable end-to-end onboarding path;
- full Shipwright runtime/resource packaging;
- Tauri per-machine NSIS packaging with normal shortcuts and uninstaller; and
- deterministic frontend and Rust unit tests.

Intentionally gated in this increment:

- production OAuth and control-plane calls;
- production device identity and signed entitlements;
- signed updater integration.

Milestone 1.5 uses clearly labeled local account/device/entitlement adapters until the control plane becomes mandatory. The real-data flow revalidates the selected file, runs extraction only in application-managed staging, and never modifies the original. The synthetic flow is rejected by the native launch gate and is labeled as non-playable throughout the UI.

## Developer verification

From this directory:

```powershell
npm.cmd install
npm.cmd run check
npm.cmd run dev:web
```

Native development requires the Visual C++ x64 build tools and a Windows SDK. Build the maintained Shipwright tools and runtime, prepare generated launcher resources, then test/package Tauri:

```powershell
$vsRoot = 'C:\Program Files\Microsoft Visual Studio\18\Community'
Import-Module (Join-Path $vsRoot 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vsRoot -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'

# Configure/build Shipwright with the repository's CMake targets.
# Generated build directories and binary resources are intentionally ignored.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\prepare-resources.ps1
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm.cmd run tauri -- build
```

These commands are developer-only. The shipped user experience must never require them.
