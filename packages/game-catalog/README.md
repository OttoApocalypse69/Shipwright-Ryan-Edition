# FTEP game catalog

`catalog.v1.json` is the provider-neutral source of game, variant, source
requirement, and runtime-candidate metadata used by FTEP. UI code must render
from this catalog instead of hard-coding game cards.

Compatibility is evidence-based. A catalog entry can describe an architectural
target without claiming that the pathway works; such entries remain
`INVESTIGATING` or `EXPERIMENTAL`. Promotion to `SUPPORTED` requires a tested
game + variant + runtime combination.

The catalog contains no game data, firmware, keys, copyrighted assets, download
locations for proprietary material, or runtime secrets.
