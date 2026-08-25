# Achievements

SRE stores achievement definitions, unlocks, and pending synchronization in local SQLite. Runtime events are evaluated locally so unlocks do not depend on network availability. A unique account/achievement constraint prevents duplicates; failed synchronization remains pending and retryable.

The control plane exposes the fourteen canonical Accord definitions plus the additive Animal Crossing expansion achievement: Treaty Ratified, Somehow This Needed OAuth, Registered Gaming Apparatus, Ahh, Zelda, Legally Supplied Bits, The Invoice Has Come Due, FICSIT Employee Onboarding, Article II Enjoyer, Diplomatic Relations Restored, PostgreSQL Was Necessary, Enterprise Gaming, Ryan Moment, Fluid Logistics, Pipeline Operational, and That Is Not Zelda.

`Ahh, Zelda` unlocks on the first playable event for any Zelda catalog title. `That Is Not Zelda` unlocks for Animal Crossing: New Horizons. `Ryan Moment` is namespaced `FICSIT-0010`. Playable evaluation enqueues an overlay notification in the same local transaction path. Tests require the path to complete under 250 ms and therefore well inside the directive's five-second user-visible window.

Approximate playable evidence is stored as approximate. It is acceptable for integrations that cannot expose game semantics, but UI and diagnostics must not call it exact.
