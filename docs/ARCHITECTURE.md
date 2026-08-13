# SRE / FTEP architecture

SRE is the local data plane. FTEP is the browser-facing control and policy plane.

```text
FTEP web/API + PostgreSQL + server-only signing keys
        | signed lease / public metadata
        v
SRE launcher services
        | catalog resolution + policy authorization
        v
sre-runtime provider contract
        |
        +-- Shipwright native adapter
        +-- Cemu-compatible external Wii U adapter
        `-- generic user-selected Switch adapter
```

The core/runtime crates have no Tauri, web, Shipwright, Cemu, Switch-runtime, or operating-system process dependency. Adapters own detection, version/source validation, preparation, configuration, launch, observation, playable signal, save location, and diagnostics. The application owns the library, authorization, sessions, achievement evaluation, overlay, and UX.

Trust boundaries:

- FTEP signs leases and release manifests with Ed25519 private keys held only by the deployment.
- SRE verifies against a release-generated bundled public-key trust set. A web response or caller cannot replace that trust anchor.
- A cached lease is bound to SRE's local random UUID identity and is checked at launch time. Policy eligibility and technical compatibility are independent.
- External runtimes and user-supplied game data are untrusted inputs. Adapters pass explicit argument arrays, validate paths, and do not download proprietary material.
- SRE is not tamper-proof DRM. Revocation blocks future authorized launches only and is non-destructive.
- Diagnostics hash sensitive paths; private keys, tokens, full paths, and game content are never exported.

The desktop overlay is a separate transparent, non-focus-stealing, click-through Tauri window. Local achievements use SQLite and synchronize through a retry queue when FTEP is available. Session state is atomically persisted in JSON. FTEP uses parameterized PostgreSQL queries and role checks at both proxy/layout and API boundaries.
