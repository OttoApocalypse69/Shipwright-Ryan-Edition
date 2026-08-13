# FTEP monorepo

Rust packages are members of the root Cargo workspace. JavaScript/TypeScript
applications and packages are members of the root pnpm workspace. The existing
CMake project remains the build root for the preserved Shipwright provider.

Root commands:

```powershell
pnpm install
pnpm dev       # Tauri launcher; native resources required
pnpm dev:web   # UI-only Vite development
pnpm build
pnpm test
pnpm lint
```

Cargo uses the root `Cargo.lock`. The pre-workspace nested launcher lockfile is
temporarily retained during the additive migration and will be removed only
after clean-machine workspace builds are established. The npm lockfile is also
retained until pnpm-based CI and packaging are proven.

Turborepo is deferred until a second real JS application or service creates a
task graph that benefits from caching and concurrent orchestration.

See [MONOREPO_MIGRATION.md](MONOREPO_MIGRATION.md) for the target tree,
dependency rules, and destructive-change boundaries.
