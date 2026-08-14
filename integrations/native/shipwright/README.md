# Shipwright adapter

This crate is the anti-corruption boundary between FTEP and the in-tree
Shipwright native runtime.

Implemented in the first Phase C slice:

- protocol/platform metadata validation;
- executable and resource-archive discovery;
- generic runtime detection and installation verification;
- atomic runtime-resource staging; and
- guarded process launch through the `RuntimeProvider` interface.

The follow-on extraction now also owns supported OoT source hashing and N64
format recognition, Torch extractor/definition discovery, cancellable asset
preparation, and the imported-asset manifest. The Tauri launcher supplies only
opaque application/resource roots and presents the adapter's results.

The generic `RuntimeProvider::prepare` operation remains unavailable because
the desktop import path needs a cancellation token and durable manifest
promotion. Call the adapter's cancellable import API instead. The adapter does
not claim semantic playable events until a trustworthy local IPC or hook
exists.
