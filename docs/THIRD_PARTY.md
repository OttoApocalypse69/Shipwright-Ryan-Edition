# Third-party runtime and integration policy

The machine-readable registry is
`packages/third-party/components.v1.json`. Every integrated runtime or runtime
dependency records its project URL, license evidence, redistribution mode,
integration type, version, attribution requirement, and source availability.

Unknown licensing is represented as `UNKNOWN` plus
`BLOCKED_PENDING_REVIEW`; it is never filled from memory or assumption. The
current Shipwright-derived root and fetched `dr_libs` entry are explicitly
blocked pending confirmation. MIT evidence is recorded for the checked-in
libultraship, Torch, and StormLib license files.

Before public release, release tooling must also generate and verify notices
for the complete Cargo, npm, CMake, vendored, fetched, and system dependency
closure. A runtime may be supported as `EXTERNAL` or `MANUAL_ONLY` without FTEP
redistributing it.
