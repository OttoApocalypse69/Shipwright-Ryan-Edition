# Adapter development

Implement `sre_runtime::RuntimeProvider` in a crate under `integrations/`. Keep runtime-specific filesystem/process rules inside the adapter and return structured `RuntimeFailure` values.

Required behavior:

1. Use a stable `RuntimeId` that appears in the catalog and runtime manifest.
2. Detect manual and known installation paths without downloading anything.
3. Validate executable/version and source layout before launch.
4. Pass executable plus argument arrays directly; do not compose shell strings.
5. Return the native process ID and observe its lifecycle.
6. Report exact or approximate playable evidence honestly.
7. Locate saves only when evidence is reliable; otherwise return unknown.
8. Redact or hash user paths in diagnostics.
9. Test success plus every failure fixture without proprietary assets.

One generic Switch provider serves all Switch catalog titles. Do not fork a provider per game unless the runtime protocol is genuinely different. External implementations are user configured and are not endorsed or redistributed by SRE.
