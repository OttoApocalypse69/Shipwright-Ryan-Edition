# Game catalog

`packages/game-catalog/catalog.v1.json` is the single catalog source for launcher and web UI. `schema.v1.json` defines its serialized contract. Metadata validation rejects duplicate IDs, unknown runtime candidates, missing source requirements, invalid compatibility values, invalid cover metadata, or drift from the required seven-title set.

Current evidence labels:

| Title | Preferred path | Status |
|---|---|---|
| Ocarina of Time | Shipwright native | Supported |
| Majora's Mask | Bundled Two Ship native runtime | Supported |
| Skyward Sword HD | managed Ryujinx Canary Switch runtime | Supported |
| Breath of the Wild | managed local Cemu runtime | Supported |
| Tears of the Kingdom | managed Ryujinx Canary runtime | Supported |
| Echoes of Wisdom | managed Ryujinx Canary runtime | Supported |
| Animal Crossing: New Horizons | managed Ryujinx Canary runtime | Supported |

Original cover metadata remains a typographic, color-safe fallback; optional HTTPS banner references are loaded remotely and copyrighted artwork is not stored in the repository. A status changes only with reproducible adapter and acceptance evidence.

Skyward Sword keeps the original Wii/Dolphin variant as a secondary compatibility
path, while the verified Switch HD variant uses the managed Ryujinx Canary runtime
automatically.
