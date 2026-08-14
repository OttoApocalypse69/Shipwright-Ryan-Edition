# Game catalog

`packages/game-catalog/catalog.v1.json` is the single catalog source for launcher and web UI. `schema.v1.json` defines its serialized contract. Metadata validation rejects duplicate IDs, unknown runtime candidates, missing source requirements, invalid compatibility values, invalid cover metadata, or drift from the required six-title set.

Current evidence labels:

| Title | Preferred path | Status |
|---|---|---|
| Ocarina of Time | Shipwright native | Supported |
| Majora's Mask | Bundled Two Ship native runtime | Supported |
| Breath of the Wild | managed local Cemu runtime | Experimental |
| Tears of the Kingdom | generic user-selected Switch runtime | Experimental |
| Echoes of Wisdom | generic user-selected Switch runtime | Investigating |
| Animal Crossing: New Horizons | generic user-selected Switch runtime | Experimental |

Original cover metadata is intentionally typographic and generated from catalog colors/marks; copyrighted box art is not stored. A status changes only with reproducible adapter and acceptance evidence.
