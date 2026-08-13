# Third-party components

`packages/third-party/components.v1.json` is the evidence registry for runtime and integration licensing/distribution. Each entry records source URL, observed version, license evidence, source availability, attribution needs, integration mode, and redistribution decision.

External Cemu-compatible and Switch runtimes are `EXTERNAL`/`MANUAL_ONLY`: SRE neither downloads nor redistributes them. It makes no warranty that a user's chosen runtime, keys, firmware, or game content is lawful in every jurisdiction.

Checked-in dependencies with repository license evidence retain their notices. Unknown evidence is represented as `UNKNOWN` and `BLOCKED_PENDING_REVIEW`, never guessed. The inherited Shipwright-derived root currently lacks a confirmed top-level license/redistribution grant, so a public bundle remains blocked until maintainers complete the full Cargo, npm, CMake, vendored, fetched, and system-dependency notice review.
