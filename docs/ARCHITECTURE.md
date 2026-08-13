# FTEP architecture

FTEP is the product and composition root. It owns library state, setup,
authorization, sessions, events, achievements, diagnostics, and presentation.
Runtime providers own only runtime-specific detection, source preparation,
verification, configuration, launch, and event fidelity.

```text
Launcher / future web UI
          |
          v
Application services (incremental)
          |
          v
ftep-core + ftep-runtime interfaces
          ^
          |
Concrete native and emulator adapters
```

The core crates cannot depend on Shipwright, Cemu, a Switch runtime, Tauri,
filesystem layout, or OS process APIs. Concrete adapters depend on the FTEP
interfaces. Catalog compatibility and runtime acquisition are data-driven and
separate from executable orchestration.

The first Phase C slice routes runtime discovery, protocol validation,
resource staging, installation verification, and process launch through
`integrations/native/shipwright`. Supported-source validation and cancellable
Torch preparation remain directly coupled to the Tauri backend until the next
behavior-preserving slice. That remaining coupling and the ordered extraction
are recorded in [MONOREPO_MIGRATION.md](MONOREPO_MIGRATION.md).

Trust boundaries and safety rules:

- the control plane is a trusted policy service, but does not issue arbitrary
  local commands;
- the launcher runs on a user-controlled device and is not tamper-proof DRM;
- runtimes are third-party executables;
- game data is user-supplied proprietary material and is never logged or
  uploaded by default;
- adapters are orchestration code with narrowly scoped filesystem/process
  authority;
- revocation refuses future FTEP-authorized launches and does nothing
  destructive.
