# Shipwright adapter

This crate is the anti-corruption boundary between FTEP and the in-tree
Shipwright native runtime.

Implemented in the first Phase C slice:

- protocol/platform metadata validation;
- executable and resource-archive discovery;
- generic runtime detection and installation verification;
- atomic runtime-resource staging; and
- guarded process launch through the `RuntimeProvider` interface.

Still owned by the launcher pending the next behavior-preserving slice:

- supported OoT source hashing/validation;
- Torch extractor discovery and cancellable asset preparation; and
- imported-asset manifest migration.

Unsupported trait methods return a structured `OPERATION_UNSUPPORTED` error.
The adapter does not claim semantic playable events until a trustworthy local
IPC or hook exists.
