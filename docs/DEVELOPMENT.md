# Development

Prerequisites: Node.js 24, pnpm 11.19, Rust 1.88 or newer, Git, and—only for the native Shipwright/installer path—Visual Studio C++ x64 tools, Windows SDK, CMake, and Ninja.

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm --filter @sre/launcher dev:web
pnpm --filter @ftep/web dev
```

`pnpm check` is the release-candidate gate. Focused commands:

```powershell
node tooling/scripts/validate-repository-metadata.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter @sre/launcher test
pnpm --filter @ftep/web test
pnpm --filter @ftep/web build
```

Tests must use synthetic fixtures and temporary directories/databases. Never commit game data, proprietary assets, firmware, keys, OAuth credentials, signing keys, `.env` values, local identities, or generated release output. Keep compatibility labels evidence based. Use parameterized SQL and direct process argument arrays.

The local web server needs a development-only `AUTH_SECRET`. PostgreSQL/OAuth/signing features additionally need variables from `apps/web/.env.example`; absence must produce an explicit unavailable state rather than a fabricated success.
