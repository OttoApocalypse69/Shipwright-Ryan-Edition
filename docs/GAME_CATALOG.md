# Game catalog

The versioned catalog lives at
`packages/game-catalog/catalog.v1.json`; its structural schema is adjacent and
Rust semantic validation lives in `ftep-core`.

Stable IDs currently scoped for end-to-end work, in priority order:

1. `zelda-oot`
2. `zelda-mm`
3. `zelda-botw`
4. `zelda-totk`

Each game contains variants, original platform, ordered runtime candidates,
source requirements, a preferred variant/runtime, and a provider-neutral
achievement namespace. File names are never authoritative game identity.

Compatibility belongs to a game + variant + runtime pathway. An architecture
slot is not evidence of support. Promotion to `SUPPORTED` requires repeatable
setup, validation, launch, and runtime-health evidence for that exact pathway.
The initial catalog therefore marks only the already verified OoT/Shipwright
path as supported.

The catalog must not contain proprietary content, console keys, firmware,
secrets, or links intended to acquire copyrighted game material.
