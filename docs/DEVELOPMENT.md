# FTEP development

Install workspace dependencies and run the normal gates from the repository
root:

```powershell
pnpm install
pnpm test
pnpm lint
pnpm build
```

Focused commands:

```powershell
pnpm --filter @ftep/launcher test
pnpm --filter @ftep/launcher build:web
node tooling/scripts/validate-repository-metadata.mjs
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Use `pnpm dev:web` for UI-only development. `pnpm dev` starts Tauri and requires
the Windows native build environment and prepared Shipwright resources described
in `apps/launcher/README.md`.

CI and tests must use synthetic fixtures. Never add ROMs, firmware, console
keys, title keys, proprietary game assets, OAuth secrets, or private device
keys to the repository or test logs.
