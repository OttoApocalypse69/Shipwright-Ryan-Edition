# Runtime provider model

`ftep-runtime` defines the provider contract. A provider exposes a stable
`RuntimeId`, kind, distribution policy, capabilities, discovery result, source
validation/preparation, installation verification/configuration, and launch.

Runtime kinds:

- `NATIVE_PORT`
- `EXTERNAL_EMULATOR`
- `MANAGED_RUNTIME`
- `EXTERNAL_RUNTIME`

Acquisition modes are separate from execution: `BUNDLED`,
`MANAGED_DOWNLOAD`, `EXTERNAL`, and `MANUAL_ONLY`. Automatic download is off in
the initial manifests. An external adapter must support a user-selected runtime
without FTEP redistributing it.

Providers advertise semantic-event, overlay, save-detection, controller,
modding, and process capabilities. FTEP must degrade honestly when an external
runtime cannot expose semantic events.

Every launch will become an FTEP session with states from `REQUESTED` through
`ENDED` or `FAILED`. Runtime events include `RUNTIME_READY`, `GAME_BOOTED`,
`PLAYABLE`, `SAVE_LOADED`, `SESSION_ENDING`, `GAME_EXITED`, achievement input,
and structured error events. `PLAYABLE` carries `EXACT` or `APPROXIMATE`
precision so process/window heuristics cannot masquerade as game semantics.

The central registry rejects duplicate provider IDs and resolves only catalog
candidates. An explicit user preference wins when it remains a valid candidate;
otherwise the configured catalog preference and priority order apply. The
application must not silently rewrite a saved working preference.

The exported synthetic provider is deterministic, has no process, accepts only
explicit fixture sources, and rejects real authorized launch mode. It supports
CI and provider-independent UI development without proprietary assets.
