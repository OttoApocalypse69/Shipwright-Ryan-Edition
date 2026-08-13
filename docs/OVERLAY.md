# Overlay

The achievement overlay is a distinct Tauri window: transparent, borderless, always on top, hidden from the taskbar, non-focus-stealing, and click-through. The launcher polls an in-memory queue, shows one notification, hides it after its declared display interval, and leaves the game window in control.

If the overlay cannot be shown, the unlock remains durable in SQLite and can still synchronize. Overlay failure never invalidates the session or entitlement.
