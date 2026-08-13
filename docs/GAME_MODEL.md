# Game model

A `GameDefinition` is a catalog record, not an executable. It has a stable game ID, franchise, display metadata, an achievement namespace, and one or more variants. Each variant declares its original platform, source requirements, runtime candidates, preferred runtime, and evidence-based compatibility.

An installation is user-local state: installation ID, game/variant/runtime IDs, source path, optional external-runtime executable, status, timestamps, and a friendly last error. Catalog definitions never contain user paths.

Entitlement answers whether FTEP policy permits a title. Compatibility answers whether a specific `game + variant + runtime` path is technically supported. Neither field may silently alter the other.
