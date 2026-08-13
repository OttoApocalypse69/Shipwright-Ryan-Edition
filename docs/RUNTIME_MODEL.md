# Runtime and session model

`sre-runtime` defines one provider contract: identity/capabilities, detect, validate version, validate source, prepare, configure, verify installation, launch, observe process, determine playable state, locate saves, and collect diagnostics.

Distribution policy is separate from execution. A provider may be bundled, managed, external, or manual-only. Current emulator-style integrations are external/manual and never download an emulator, keys, firmware, or game content.

Every launch creates a `RuntimeSession` containing stable game, variant, runtime, device, timing, process, exit, duration, and result fields. The state machine is:

```text
REQUESTED -> PREPARING -> LAUNCHING -> RUNNING -> PLAYABLE -> ENDED
                         `-------------------------------> FAILED
```

Playable evidence is `EXACT` when the provider exposes a semantic signal and `APPROXIMATE` when process/runtime observation crosses a documented threshold. Approximation is labelled; it never masquerades as semantic telemetry. A missing playable signal leaves the session running and does not invent an achievement.

Synthetic providers are test-only, accept explicit fixture paths, cannot launch real authorized sessions, and cover missing source, wrong version, broken install, launch failure, slow startup, and no-playable-signal cases.
