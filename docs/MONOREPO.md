# Monorepo guide

The repository has three build graphs: pnpm for TypeScript applications, Cargo for Rust packages, and the preserved upstream CMake build for Shipwright resources.

- The root `pnpm-lock.yaml` is authoritative for JavaScript dependencies.
- The root `Cargo.lock` is authoritative for all Rust workspace members.
- `pnpm check` runs lint, formatting/clippy, unit/integration tests, metadata validation, TypeScript/Next builds, and Rust builds.
- CMake remains isolated under the existing root targets; SRE consumes prepared binaries/resources and does not rewrite upstream sources.

Dependency direction is `apps -> services/policy -> core/runtime`, with concrete adapters implementing `sre-runtime`. Core crates may not depend on apps or integrations. The web app shares the catalog JSON as data and does not import launcher implementation code.

Generated directories (`target`, `.next`, `dist`, native build trees, `release-assets`) are not source. Do not add ROMs, firmware, console keys, title keys, game assets, OAuth secrets, signing private keys, or local device identities anywhere in the worktree.
