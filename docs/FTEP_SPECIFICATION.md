# FICSIT Treaty Enforcement Platform Specification

- **Document status:** Normative product and architecture specification
- **Specification version:** 2.0
- **Treaty ID:** `FICSIT-TREATY-0001`
- **Primary target:** Windows x64
- **Host runtime:** Ship of Harkinian (Shipwright Ryan Edition)

## 1. Product doctrine

> If Ryan has to use a terminal, we have failed.

FTEP is a polished consumer launcher attached to an intentionally excessive treaty and compliance platform. A normal user must never need to run a shell command, edit JSON, set an environment variable, copy files into an internal directory, compare hashes, understand OAuth, manage a device certificate, or understand Shipwright internals.

This document uses **MUST**, **MUST NOT**, **SHOULD**, and **MAY** as normative requirements.

The complete happy path is deliberately short:

1. Download the installer.
2. Click through setup.
3. Log in.
4. Accept the treaty.
5. Select legally supplied game data.
6. Click **Play Zelda**.

Everything reasonably automatable between those actions MUST be automated.

### 1.1 Product boundaries

- FTEP MUST NOT distribute, locate, link to, or search for copyrighted Nintendo game data.
- FTEP MUST import only game data selected and supplied by the user.
- The treaty is a humorous platform agreement/interpersonal treaty, not a claim of a legally binding real-world contract.
- Entitlement enforcement MUST be confined to the FTEP-controlled launcher and runtime.
- FTEP MUST NOT damage files, destroy saves, uninstall software, terminate unrelated processes, execute arbitrary remote commands, or interfere with the operating system.
- FTEP MUST NOT inspect arbitrary running programs to infer Satisfactory activity.
- FTEP MUST remain conventionally uninstallable and MUST NOT install hidden persistence or unrelated background services.
- Real-life obligations and emergencies always override the treaty.

## 2. System architecture

FTEP SHOULD be implemented as an additive layer around upstream Shipwright so that the Zelda runtime remains easy to rebase and compare with upstream.

### 2.1 Repository layout

The target layout is:

```text
apps/
  launcher/          Tauri 2 + React + TypeScript desktop application
  web/               Next.js App Router website for Vercel
  control-plane/     HTTP API and background jobs
packages/
  contracts/         Versioned schemas, API types, event names, error catalog
  treaty/            Canonical treaty text and version metadata
  achievements/      Data-driven achievement definitions
  release/           Release-manifest schema and signing/verification helpers
soh/soh/Ftep/        Small C++ runtime integration boundary
docs/                Architecture, operations, privacy, and this specification
```

Tauri 2 + React is the preferred launcher architecture because it provides a modern Windows GUI, a native file picker, a signed updater, and an installer without embedding an entire browser runtime. The launcher MUST communicate with Shipwright through a narrow, versioned boundary rather than coupling web UI code to game internals.

The in-game achievement renderer MUST remain native C++/ImGui. The repository already exposes `GameInteractor::IsPlayerInControl()` and frame/player hooks; the first-playable detector SHOULD build on those APIs. The existing `Notification` system is the preferred rendering foundation, extended with an achievement-specific queued card rather than duplicated by a second overlay stack.

### 2.2 Components

- **Launcher:** setup, login, treaty acceptance, import, pre-flight checks, launch, achievements, updates, and friendly diagnostics.
- **Zelda runtime adapter:** emits trusted local gameplay events, renders achievement toasts, and reads only the minimum launcher-issued runtime context.
- **Control plane:** accounts, treaty versions and acceptance, devices, entitlement state, leases, sessions, achievement sync, and admin operations.
- **Website:** public product/treaty/status/download pages plus authenticated user and admin experiences.
- **PostgreSQL:** canonical server-side records. Local functionality MUST NOT require a live database connection.
- **Local durable store:** launcher/runtime state, validated import metadata, pending achievement events, unlocks, and last-known signed lease. Secrets MUST use the Windows credential store; non-secret structured state SHOULD use SQLite.

### 2.3 Trust boundaries

- A launcher update is trusted only after its signed metadata and artifact digest are verified.
- A runtime lease is trusted only after signature, audience, device, version, and expiry checks pass.
- Game-data validation runs locally. Raw game data, extracted copyrighted assets, hashes that would unnecessarily fingerprint the user's copy, and save data MUST NOT be uploaded.
- Achievement unlocks are locally authoritative for presentation and asynchronously reconciled with the server.
- Admin actions MUST be authenticated, authorized, audited, and restricted to defined domain operations. There is no general remote-command facility.

## 3. Installation and first run

### 3.1 Windows installer

FTEP MUST ship as a normal signed Windows installer. The default target is `C:\Program Files\FTEP`. Setup provides:

- an editable install location;
- enabled-by-default desktop and Start Menu shortcuts;
- an enabled-by-default automatic update check;
- a visible install action and progress;
- standard Windows uninstall registration;
- an installation summary for the launcher, Zelda runtime, treaty services, updater, and diagnostics;
- a final **Begin Diplomatic Onboarding** action.

Per-machine installation may request elevation once through the normal Windows consent dialog. A future per-user install MAY avoid elevation, but MUST preserve conventional uninstall behavior.

The updater MAY run when the launcher runs. A persistent update service MUST NOT be installed unless a future, separately reviewed requirement makes one necessary and the user explicitly opts in.

### 3.2 First-run state machine

The canonical onboarding sequence is:

```text
WELCOME
  -> ACCOUNT LOGIN
  -> THE GREAT ZELDA–SATISFACTORY ACCORDS
  -> USER ACCEPTANCE
  -> GAME DATA SELECTION
  -> GAME DATA VALIDATION
  -> ASSET IMPORT
  -> DEVICE REGISTRATION
  -> ENTITLEMENT ACTIVATION
  -> CONTROLLER CHECK
  -> GRAPHICS CHECK
  -> SYSTEM DIAGNOSTICS
  -> READY
  -> PLAY ZELDA
```

Every stage MUST provide:

- a clear title and short plain-language explanation;
- overall and stage progress;
- **Back** whenever reversing is safe;
- a single primary **Continue**, **Retry**, or completion action;
- bounded automatic retry with exponential backoff for recoverable failures;
- expandable **Advanced Details**;
- a copyable, redacted diagnostic error;
- original FICSIT-flavored commentary;
- persistent checkpoints so closing and reopening resumes safely.

Implementation terms such as PKCE, certificate, manifest signature, hash, JSON, API route, and SQLite MUST stay inside **Advanced Details**.

Back navigation MUST NOT silently revoke a completed treaty acceptance, invalidate an imported asset archive, or create duplicate devices. Destructive replacement actions require a plain-language confirmation.

### 3.3 Accessibility and controller behavior

- All launcher functions MUST be keyboard accessible and have visible focus states.
- Text and controls MUST satisfy WCAG 2.2 AA contrast and scaling expectations.
- Motion MUST respect the operating system's reduced-motion preference.
- Color MUST NOT be the only status signal.
- Toasts MUST not take gameplay focus or require dismissal.
- Launcher flows SHOULD be controller operable after the controller-check stage.
- Technical details and error copy controls MAY remain keyboard/mouse optimized.

## 4. Account, device, and entitlement experience

### 4.1 Login

The launcher SHOULD use the operating system browser with OAuth/OIDC Authorization Code + PKCE. The user sees **Log in**, the provider's consent page, and a success return to FTEP; protocol vocabulary is not shown in the default UI.

Tokens MUST be scoped, short-lived where practical, and stored in Windows Credential Manager. Refresh tokens MUST NOT be written to plaintext configuration. Login success emits `ACCOUNT_LOGIN_SUCCEEDED` and can unlock **Somehow This Needed OAuth**.

### 4.2 Device registration

The launcher generates a non-exportable device signing key when platform support permits, registers the public key, and assigns a friendly name such as `RYAN-PC-01`. Device registration MUST be idempotent and recoverable. Device certificates and identifiers are managed automatically.

Successful first registration emits `DEVICE_REGISTERED` and can unlock **Registered Gaming Apparatus**.

### 4.3 Entitlements and leases

The control plane evaluates treaty state and issues a signed, time-bounded launcher lease. The launcher verifies the lease locally and passes the least amount of information needed to the runtime. Temporary network loss MUST NOT interrupt an already authorized active game session.

The grace/offline policy MUST be explicit and configurable. The default SHOULD allow the last valid lease to remain usable until its signed expiry. Clock rollback and device mismatch produce a friendly diagnostic; they MUST NOT trigger punitive system behavior.

## 5. User-provided game data

### 5.1 Selection

The launcher MUST provide a native file picker headed **Locate Your Zelda Game Data** and state:

> FTEP does not provide Nintendo game data. Select the compatible game data you already possess and FTEP will handle everything else.

There MUST NOT be a button or link that searches the internet for copyrighted game data.

### 5.2 Validation and import

Validation is a staged, cancellable local job:

1. confirm the file can be opened read-only;
2. identify the format without relying on the filename;
3. match a supported version using the repository's supported-hash data;
4. perform integrity checks;
5. verify the matching import/extraction pipeline exists;
6. calculate required free space before extraction;
7. import into an application-managed staging directory;
8. atomically promote the completed asset archive;
9. clean partial staging data after failure or cancellation.

The normal UI displays a checklist for file readability, recognized format, supported version, valid integrity, and available import pipeline. Technical values and hashes stay in **Advanced Details**.

Unsupported data MUST produce `FICSIT-0007 GAME_DATA_NOT_SUPPORTED`, explain that the supplied variant is unsupported, and offer **Technical Details** and **Choose Another File**. It MUST NOT imply that the user should download another copy.

The imported archive and original selected file MUST never be deleted or overwritten without explicit user confirmation. The launcher SHOULD remember the original path only as needed to support re-import and MUST tolerate that path later disappearing.

## 6. Pre-flight inspection

Pre-flight runs before first launch, after a runtime/update change, after imported asset state changes, and on demand from diagnostics. It checks:

- supported OS and CPU architecture;
- GPU and graphics API availability;
- required free disk space;
- launcher and runtime files;
- imported asset presence and integrity;
- controller presence and usable mapping;
- writable save directory;
- control-plane connectivity;
- current authentication;
- device registration;
- entitlement state;
- lease signature, audience, device, and expiry;
- launcher/runtime protocol compatibility.

Each check returns `PASS`, `WARNING`, `FAIL`, or `SKIPPED`, plus a stable error code, plain-language message, remediation action, retry policy, redacted details, and elapsed time.

Only launch-critical failures disable **Play Zelda**. A missing controller is a warning when keyboard input remains available. A network failure is a warning when a valid offline lease exists. Diagnostics MUST never report **Ready** when a required runtime, asset archive, writable save directory, or valid entitlement is absent.

The ready screen presents:

```text
FICSIT PRE-FLIGHT INSPECTION

Runtime             PASS
Game data           PASS
Save directory      PASS
Graphics            PASS
Controller          PASS
Network             PASS
Device identity     PASS
Treaty entitlement  PASS

SYSTEM STATUS: READY FOR ZELDA OPERATIONS

[ PLAY ZELDA ]
```

## 7. Achievement domain

FTEP achievements are independent of Steam or any platform achievement API.

### 7.1 Entities

```text
AchievementDefinition
  id, version, name, description, icon, score, category, hidden,
  triggerKind, triggerConfig, activeFrom, retiredAt

AchievementTrigger
  id, achievementId, eventType, predicate, progressRule

AchievementEvent
  eventId, eventType, accountId, deviceId, occurredAt, payload,
  source, schemaVersion

AchievementProgress
  accountId, achievementId, current, target, updatedAt, version

AchievementUnlock
  unlockId, accountId, achievementId, occurredAt, sourceEventId,
  deviceId, syncState
```

Definitions MUST be data-driven and versioned. Gameplay code emits stable domain events; it MUST NOT contain scattered per-achievement prose or conditions.

Recommended server tables are `achievements`, `achievement_triggers`, `achievement_unlocks`, and `achievement_progress`. The server MUST enforce a unique `(account_id, achievement_id)` unlock. Event ingestion MUST be idempotent by `event_id`.

The launcher/runtime local store mirrors definitions, unlocks, progress, and an outbox. A local transaction records an unlock and its outbox item before presentation. Sync retries asynchronously and never blocks a toast.

### 7.2 Event contract

Initial event names include:

```text
ACCOUNT_LOGIN_SUCCEEDED
TREATY_ACCEPTED
GAME_DATA_VALIDATED
DEVICE_REGISTERED
GAMEPLAY_READY
ARTICLE_II_ACTIVATED
ZELDA_SESSION_STARTED
SATISFACTORY_SESSION_RECORDED
SATISFACTORY_SESSION_QUALIFIED
TREATY_STATE_CHANGED
DATABASE_MIGRATION_APPLIED
PLATFORM_HEALTHY
RYAN_MOMENT_CLASSIFIED
HYDRATION_ACKNOWLEDGED
```

Events MUST contain no raw game data, save data, medical information, or access token. Timestamps use UTC on the wire and a monotonic clock for local deadlines.

### 7.3 Mandatory first achievement

The canonical definition is:

```json
{
  "id": "FTEP-ACH-0001",
  "name": "Ahh, Zelda",
  "description": "Hope you had fun playing. Now onto Satisfactory.",
  "category": "Zelda",
  "score": 100,
  "trigger": "FIRST_PLAYABLE_SESSION",
  "hidden": false
}
```

**Ahh, Zelda is a release-blocking invariant.** It MUST be displayed on Ryan's first genuinely playable Zelda session, no later than five seconds after player control first becomes available.

The runtime integration contract is:

1. register a Shipwright game-frame or player-update hook;
2. wait until a normal save is loaded and `GameInteractor::IsPlayerInControl()` is true;
3. detect the false-to-true edge for the current launched session;
4. emit `GAMEPLAY_READY` exactly once for that session using a durable unique event ID;
5. if the account has no local unlock for `FTEP-ACH-0001`, persist the unlock and outbox entry atomically;
6. wait a configurable dramatic delay with a default of two seconds and a hard maximum that preserves the five-second deadline;
7. enqueue the local toast even when offline;
8. synchronize to the control plane asynchronously.

Process startup, title-screen entry, file selection, save-load initiation, cutscenes that prevent control, and a mere `OnLoadGame` callback are insufficient by themselves.

The deadline is measured from the first frame for which the playable predicate becomes true to the first frame on which the toast is visibly rendered. Default delay is two seconds, toast lifetime is seven seconds, and deadline budget is:

```text
playable edge -> durable local write -> delay -> first visible toast frame <= 5.0 s
```

If local persistence fails, the runtime MUST still display the toast before the deadline and queue a prominent non-blocking diagnostic; it SHOULD retry persistence. Server failure can never suppress the joke.

The unlock normally occurs once per account. Local deduplication uses the authenticated account ID; server reconciliation resolves multi-device races through the unique account/achievement constraint.

### 7.4 Toast renderer

The in-game achievement toast MUST support:

- title, description, original icon, optional score, and optional category;
- animated entrance and dismissal with reduced-motion alternatives;
- a FIFO queue so unlocks are never visually stacked into unreadability;
- configurable top-left, top-right, bottom-left, or bottom-right placement;
- configurable duration with a safe default of seven seconds;
- safe-area margins and scale-aware, high-contrast typography;
- controller-safe, non-interactive presentation that never steals focus;
- a muted or original, non-copyrighted sound; and
- deterministic test hooks for time and animation state.

Categories are `Treaty`, `Zelda`, `Satisfactory`, `Infrastructure`, `Diplomacy`, `Administrative`, and `Secret`.

The mandatory toast reads:

```text
ACHIEVEMENT UNLOCKED

Ahh, Zelda

Hope you had fun playing.
Now onto Satisfactory.

+100 FICSIT POINTS
```

### 7.5 Seed achievements

| ID | Name | Description | Trigger |
|---|---|---|---|
| `FTEP-ACH-0001` | Ahh, Zelda | Hope you had fun playing. Now onto Satisfactory. | First `GAMEPLAY_READY` for the account |
| `FTEP-ACH-0002` | Treaty Ratified | You actually agreed to this. | Treaty acceptance completes |
| `FTEP-ACH-0003` | Somehow This Needed OAuth | You wanted Zelda. We built identity infrastructure. | First successful account login |
| `FTEP-ACH-0004` | Legally Supplied Bits | FTEP has detected a completely user-provided collection of data. | Successful game-data validation |
| `FTEP-ACH-0005` | Registered Gaming Apparatus | Your computer is now recognized by the Interpersonal Treaty Authority. | First successful device registration |
| `FTEP-ACH-0006` | The Invoice Has Come Due | Article II would like a word. | First Zelda session after Article II becomes active |
| `FTEP-ACH-0007` | FICSIT Employee Onboarding | Welcome to the factory. Your free time has been processed. | First qualifying Satisfactory multiplayer session |
| `FTEP-ACH-0008` | Article II Enjoyer | Industrial cooperation has improved diplomatic relations. | Complete a full qualifying Satisfactory session |
| `FTEP-ACH-0009` | Diplomatic Relations Restored | Zelda privileges restored. Try not to ruin this. | Transition from `MATERIAL_BREACH` or `SUSPENDED` to `COMPLIANT` |
| `FTEP-ACH-0010` | PostgreSQL Was Necessary | It absolutely was not. | First backend migration in development/admin environments |
| `FTEP-ACH-0011` | Enterprise Gaming | A distributed system was deployed so two people could play video games. | Control plane, launcher, and entitlement system all healthy |
| `FTEP-ACH-0012` | Ryan Moment | Engineering could not have reasonably anticipated this. | Explicit `FICSIT-0010` classification only |
| `FTEP-ACH-0013` | Fluid Logistics | Biological machinery also requires input buffers. | First hydration acknowledgement |
| `FTEP-ACH-0014` | Pipeline Operational | Water successfully delivered to operator. | Five hydration acknowledgements |

`Ryan Moment` MUST NOT be inferred from an ordinary crash, validation failure, network error, unsupported file, or product bug. It requires an explicit authorized classification with an audit note.

### 7.6 Achievement pages

The launcher and authenticated website expose total unlocked/available, percentage, recent unlocks, category filters, unlocked cards, locked cards, and hidden placeholders. Hidden achievements reveal neither name nor description until unlocked unless their definition explicitly permits it.

The launcher MUST work from local state when offline. The website uses canonical synchronized state and shows the last synchronization time when relevant.

## 8. The Great Zelda-Satisfactory Accords

### 8.1 Versioning and acceptance

The canonical title is **THE GREAT ZELDA–SATISFACTORY ACCORDS** and its stable ID is `FICSIT-TREATY-0001`.

Each published version stores immutable canonical UTF-8 text, semantic version, publication time, and SHA-256 hash. Acceptance records contain treaty ID, treaty version, account ID, acceptance timestamp, accepted text hash, and audit metadata. An acceptance is valid only when its recorded hash matches the canonical bytes of that version.

Material amendments require a new version and a new explicit acceptance. Formatting-only corrections MAY retain acceptance only when the canonical accepted bytes and meaning are unchanged; otherwise version again.

The onboarding UI shows a short human summary, a scrollable full-text view, an unchecked acknowledgement, and **Ratify Treaty**. Ratification stays disabled until the acknowledgement is checked. The default UI MUST say that this is a platform agreement/interpersonal treaty and not a legally binding real-world contract.

Human summary:

```text
Provider: Teri
Beneficiary: Ryan

Teri:
  Makes supported Zelda access work.

Ryan:
  Plays qualifying Satisfactory sessions.
  Does not deliberately destroy the factory.
  Does not rage quit solely over normal game mishaps.
  Does not deliberately waste everyone's time.
```

### 8.2 Canonical Treaty Version 2

#### ARTICLE I — TECHNICAL PROVISION

Teri agrees to provide reasonable technical assistance necessary to establish and maintain FTEP-supported Zelda access.

This does not require Teri to provide copyrighted game data.

#### ARTICLE II — INDUSTRIAL COOPERATION

Ryan agrees to participate in mutually scheduled Satisfactory multiplayer sessions with Teri in good faith.

A qualifying session should normally include active participation rather than merely joining the server and going AFK. The default qualifying threshold is 30 minutes of active participation and MUST be configurable.

#### ARTICLE III — WORLD PRESERVATION

Ryan shall not intentionally damage, destroy, dismantle, corrupt, or materially degrade the shared Satisfactory world.

Prohibited intentional actions include:

- dismantling productive infrastructure without agreement;
- deliberately severing important logistics;
- intentionally collapsing the electrical grid;
- intentionally starving critical production chains;
- deliberately wasting stored high-value materials;
- deliberately destroying vehicles or equipment;
- deliberately causing unrecoverable inventory loss;
- deleting or corrupting save data;
- deliberately obstructing construction projects; and
- intentionally making the factory materially worse merely for amusement.

Accidental mistakes are not treaty violations. Experimental engineering is permitted when undertaken in good faith.

#### ARTICLE IV — TOKEN CONSERVATION AND REMEDIATION

The parties acknowledge that repairing unnecessary damage may require real human time and may result in additional AI, tool, or token usage with real-world cost.

Therefore Ryan shall not intentionally create avoidable technical or gameplay damage merely to force Teri to repair it.

This article does not create monetary debt or financial liability. The remedy is restoration of the affected world state and reasonable assistance fixing what was damaged.

#### ARTICLE V — NO RAGE-QUIT PROTOCOL

Ryan shall not intentionally terminate a mutually agreed qualifying session solely because:

- a production line failed;
- a creature killed him;
- Teri made fun of his conveyor layout;
- a train signal behaved as train signals do;
- the factory briefly lost power; or
- someone said “just one more production line.”

When reasonably possible, either party wishing to end a session should communicate that intention before leaving.

Exceptions include internet failure, power failure, crashes, illness, school or work obligations, family matters, emergencies, and genuine real-life constraints. Real life overrides the treaty.

#### ARTICLE VI — GOOD-FAITH PARTICIPATION

Participation intended solely to satisfy the timer while avoiding actual cooperation does not count.

Examples include joining and going AFK, walking into a wall for 30 minutes, leaving the game running unattended, or actively sabotaging while technically remaining connected.

A session should involve reasonable cooperative gameplay.

#### ARTICLE VII — INDUSTRIAL CHANGE CONTROL

Major destructive changes to established infrastructure should be discussed before execution. Examples include a factory-wide redesign, power architecture replacement, rail-network demolition, mass conveyor replacement, destruction of central storage, or relocation of strategically important facilities.

Normal construction does not require approval. Do not turn gameplay into bureaucracy. The purpose is exclusively to prevent catastrophic Ryan Events.

#### ARTICLE VIII — SAVE INTEGRITY

Neither party shall intentionally:

- delete the canonical shared save;
- overwrite it with an older save for malicious reasons;
- corrupt the save;
- modify backups to conceal sabotage; or
- interfere with backup automation.

Automatic backups should be enabled where practical.

#### ARTICLE IX — REMEDIATION

If either party accidentally causes substantial damage, they should:

1. report what happened;
2. not conceal it;
3. help assess the damage;
4. restore the affected infrastructure where reasonably practical; and
5. continue playing.

Accidents are not breaches when handled in good faith.

#### ARTICLE X — SESSION SCHEDULING

Neither party is expected to be available continuously. Sessions are mutually scheduled.

The treaty does not override sleep, work, school, health, emergencies, or other real-life obligations.

#### ARTICLE XI — HYDRATION PROTOCOL

Industrial personnel are strongly encouraged to maintain hydration during extended gaming sessions.

For sessions exceeding approximately 90 minutes, the launcher MAY display:

```text
FICSIT OCCUPATIONAL HYDRATION NOTICE

Water remains compatible with continued industrial operations.

[ I HAVE ACQUIRED WATER ]
[ REMIND ME LATER ]
```

Hydration achievements may exist. Failure to acknowledge or drink water MUST NOT suspend Zelda entitlement. FTEP is overengineering a joke, not establishing a dystopian hydration police force.

#### ARTICLE XII — MUTUAL ANTI-IDIOT CLAUSE

Neither party shall deliberately exploit technical ambiguities in this treaty in a manner obviously contrary to its purpose.

For example: “I joined Satisfactory for exactly thirty minutes but spent all thirty minutes jumping into the void.” No. The standard is reasonable good faith.

#### ARTICLE XIII — ENTITLEMENT REMEDIES

Failure to satisfy Article II may result in:

```text
COMPLIANT -> WARNING -> MATERIAL_BREACH -> ZELDA_ENTITLEMENT_SUSPENDED
```

Suspension affects only the FTEP-controlled launcher entitlement. It shall never damage files, destroy saves, uninstall applications, remotely terminate unrelated processes, execute arbitrary commands, or interfere with the operating system.

#### ARTICLE XIV — RESTORATION OF PRIVILEGES

When compliance is restored:

```text
MATERIAL_BREACH
  -> REMEDIATION_COMPLETED
  -> COMPLIANT
  -> ZELDA PRIVILEGES RESTORED
```

The launcher displays a **Treaty Relations Normalized** notice, confirms that Article II obligations are satisfied and Zelda runtime authorization is restored, thanks Ryan for renewed industrial cooperation, and offers **Play Zelda**.

### 8.3 Treaty state machine

```text
PENDING_ACCEPTANCE
  -> COMPLIANT
  -> WARNING
  -> MATERIAL_BREACH
  -> ZELDA_ENTITLEMENT_SUSPENDED
  -> REMEDIATION_IN_PROGRESS
  -> COMPLIANT
```

Transitions MUST be explicit domain operations with actor, reason, timestamp, prior state, next state, and audit record. The state model MUST distinguish `MATERIAL_BREACH` from the entitlement projection `ZELDA_ENTITLEMENT_SUSPENDED` even if both are presented together.

A return from `MATERIAL_BREACH` or suspension to `COMPLIANT` emits `TREATY_STATE_CHANGED` and can unlock **Diplomatic Relations Restored**.

## 9. Session reporting and hydration

Version one permits Teri to manually record a Satisfactory session. A record includes participants, scheduled start, actual start/end, active minutes, qualifying threshold at the time, qualification result, optional plain-language note, and recording administrator.

Manual reports MUST be editable through audited corrections. They MUST NOT require spyware, process enumeration, screen capture, arbitrary file inspection, or background surveillance. A future integration may use legitimate, consented APIs, but manual reporting remains an acceptable privacy-preserving path.

The default qualifying threshold is 30 active minutes. Merely being connected is insufficient; qualification includes an explicit good-faith confirmation. The system is not expected to algorithmically judge humor, sabotage, or human intent.

Hydration reminders are local, optional, self-reported, and non-punitive. The default first reminder is after 90 minutes of the current session. **I Have Acquired Water** increments only an acknowledgement count. FTEP MUST NOT collect health, medical, volume-consumed, or biometrics data. Hydration state MUST NOT affect entitlement.

## 10. Launcher experience

After onboarding, the `FTEP` desktop shortcut opens the launcher directly. A returning-user dashboard presents:

```text
FICSIT TREATY ENFORCEMENT PLATFORM

Good afternoon, Ryan.

Treaty       COMPLIANT
Game Data    VERIFIED
Entitlement  ACTIVE
Runtime      READY

[ PLAY ZELDA ]

Achievements: 4 / 37

Industrial diplomacy through excessive software engineering.
```

The launcher navigation SHOULD contain **Home**, **Achievements**, **Treaty**, **Diagnostics**, and **Settings**. Account, update, import replacement, log export, privacy, and uninstall guidance belong in Settings or Diagnostics rather than the primary play path.

Errors display a stable FICSIT code, plain-language cause, one recommended action, retry where safe, and expandable redacted details. **Copy Diagnostic** MUST exclude tokens, raw game-data paths by default, device private material, and personal data not required to troubleshoot.

## 11. Public website and account portal

`apps/web` is a modern Next.js App Router application deployable to Vercel. It MUST have:

```text
/
/download
/status
/achievements
/treaty
/login
/dashboard
/admin
/privacy
/security
```

Public pages are Home, Download, Status, Treaty, Privacy, and Security. Achievements, Dashboard, and Admin require authentication; Admin additionally requires an admin role.

The home page leads with:

```text
FICSIT TREATY ENFORCEMENT PLATFORM

Enterprise-grade infrastructure
for unnecessarily complicated gaming agreements.

[ DOWNLOAD FOR WINDOWS ]
[ VIEW THE TREATY ]
```

Ryan's dashboard shows treaty status, Zelda entitlement, current lease remaining, qualifying Satisfactory session count, achievement count, device name, and actions for download, achievements, and treaty.

Teri's **Treaty Operations Center** shows beneficiary count, active entitlements, material breaches, qualifying sessions, and explicitly classified Ryan Moments. Counts MUST link to filtered, authorized detail views rather than becoming unauditable decorative numbers.

### 11.1 Status page

Status reports control-plane API, authentication, entitlement issuance, achievement synchronization, and release metadata. It MUST distinguish measured service health from cached/unknown state and MUST NOT leak internal hostnames, stack traces, or personal account data.

### 11.2 Download page

GitHub Releases is the canonical binary source. Vercel MUST NOT duplicate release binaries.

The page resolves the latest stable release and presents version, platform, installer download, SHA-256, publication date, release notes, and verification help. Expected assets are:

```text
FTEP-Setup-x64.exe
FTEP-Portable-x64.zip
SHA256SUMS.txt
release-manifest.json
release-manifest.sig
```

Release metadata SHOULD be obtained from GitHub's Releases API through a server-only route and cached with incremental revalidation (default 15 minutes) plus stale-if-error behavior. A single page render MUST NOT make one GitHub request per component. Downloads redirect to the canonical GitHub asset URL.

The web application MUST verify the signed release manifest before describing an artifact as verified. A missing or invalid manifest produces an unavailable verification state, not a green check.

## 12. Release, installer, and update security

### 12.1 Pipeline

A stable tag runs a GitHub Actions workflow that:

1. checks out the immutable tag;
2. builds the launcher and pinned Shipwright runtime;
3. runs unit, integration, UI, and runtime tests;
4. packages the Windows installer and portable archive;
5. computes SHA-256 and byte size for every artifact;
6. creates a canonical JSON release manifest;
7. signs the exact manifest bytes;
8. verifies the signature and every artifact digest in a separate step;
9. publishes one GitHub Release with all expected assets;
10. allows the website cache to discover the release automatically.

Publishing MUST fail closed if tests, signing, manifest verification, or asset verification fails. The site MUST never require a manual version edit.

### 12.2 Signed manifest

The canonical manifest schema includes:

```json
{
  "schema_version": 1,
  "version": "0.4.2",
  "channel": "stable",
  "published_at": "2026-08-13T00:00:00Z",
  "runtime_protocol": 1,
  "artifacts": [
    {
      "platform": "windows-x64",
      "name": "FTEP-Setup-x64.exe",
      "sha256": "...",
      "size": 12345678
    }
  ]
}
```

Canonical JSON serialization MUST be deterministic. The manifest SHOULD use Ed25519/minisign-compatible signatures so the Tauri updater and custom verifier can share a small, auditable primitive. The private signing key lives only in protected release automation; the launcher contains one or more pinned public keys and an explicit rotation mechanism.

The launcher MUST verify signature, channel, semantic version, runtime protocol, platform, artifact name, digest, size, and anti-rollback policy before trusting metadata or executing an installer. TLS is necessary but not a substitute for signature verification.

Interrupted downloads resume only when the server and local partial metadata agree. Installation occurs only after full digest verification. Update failure leaves the current working installation launchable.

## 13. Data and API requirements

The control plane SHOULD expose versioned HTTPS JSON endpoints with generated shared schemas. Mutating requests use idempotency keys where retries are expected.

Minimum server records are:

- accounts and roles;
- registered devices and revoked-device state;
- treaty definitions, versions, acceptances, and state transitions;
- entitlement decisions and signed lease issuance metadata;
- reported Satisfactory sessions and corrections;
- achievement definitions, triggers, events, progress, and unlocks;
- release channels and optional cached metadata;
- audit events for privileged operations.

The control plane MUST NOT store raw Zelda game data, extracted assets, saves, launcher secrets, device private keys, or hydration/medical information.

Every externally visible state mutation MUST be attributable and time-stamped. Logs use correlation IDs and structured error codes, redact secrets, and apply documented retention. Admin access and device revocation require additional confirmation.

## 14. Visual design

The design combines industrial corporate software, aerospace command-and-control, restrained absurd bureaucracy, and modern consumer-launcher usability.

The brand MUST be original. It MAY use hazard-inspired amber, charcoal, off-white, grid lines, inspection stamps, technical typography, and restrained motion, but MUST NOT copy Satisfactory logos, UI panels, icons, sound effects, textures, or other copyrighted assets.

Humor belongs in commentary and achievement copy. Primary actions, failures, privacy choices, and entitlement consequences remain unambiguous.

## 15. Privacy and safety

- Collect the minimum data required for accounts, devices, treaty state, sessions, entitlements, and achievements.
- Publish a plain-language privacy page before any public beta.
- Provide account session/device review and device revocation.
- Never upload user game data, extracted assets, saves, arbitrary process lists, screenshots, or filesystem inventories.
- Never treat hydration acknowledgements as health data or an entitlement input.
- Do not use invasive anti-cheat or background surveillance.
- Diagnostic bundles require explicit user action, preview, redaction, and consent before upload.
- Security documentation MUST describe update verification, credential storage, data boundaries, vulnerability reporting, and supported versions.

## 16. Error catalog

Error codes are stable, searchable, and safe to copy. Initial reserved codes include:

| Code | Meaning | Default action |
|---|---|---|
| `FICSIT-0001` | Unexpected internal failure | Retry, then copy redacted diagnostic |
| `FICSIT-0002` | Control plane unavailable | Retry automatically; use valid offline lease if possible |
| `FICSIT-0003` | Authentication expired or invalid | Log in again |
| `FICSIT-0004` | Device registration failed | Retry registration |
| `FICSIT-0005` | Entitlement or lease invalid | View treaty status or refresh entitlement |
| `FICSIT-0006` | Asset import failed | Preserve original; retry clean staging import |
| `FICSIT-0007` | Game data not supported | Choose another legally supplied file |
| `FICSIT-0008` | Runtime/launcher incompatible | Install verified update |
| `FICSIT-0009` | Pre-flight requirement failed | Run offered remediation |
| `FICSIT-0010` | Extraordinary user-induced failure | Explicit admin classification only; never automatic |

## 17. Incremental roadmap amendment

These milestones are inserted into the existing roadmap without renumbering its original milestones.

### Milestone 1.5 — Ryan-Proof GUI

Deliver before remote services become mandatory:

- graphical Windows installer;
- graphical resumable onboarding shell;
- native game-data picker and local validation;
- atomic asset import;
- pre-flight diagnostics;
- returning-user launcher dashboard;
- synthetic-data/demo path that requires no copyrighted data and cannot be mistaken for a playable retail import.

**Exit criteria:** A clean Windows VM can install, complete the synthetic onboarding path, diagnose expected failures, uninstall conventionally, and perform no terminal/manual-config step.

### Milestone 2.5 — Achievements

Deliver:

- versioned achievement definitions and event bus;
- local durable unlock/progress/outbox persistence;
- queued in-game toast renderer;
- first-playable-session detector using the runtime control predicate;
- release-blocking **Ahh, Zelda** achievement;
- idempotent asynchronous server synchronization;
- deterministic timing tests for the five-second requirement.

**Exit criteria:** An offline deterministic runtime test crosses the playable edge and visibly begins the mandatory toast within five seconds exactly once, survives restart, and later synchronizes idempotently.

### Milestone 4.5 — Treaty v2

Deliver:

- canonical fourteen-article text and immutable versioning;
- human summary and scrollable acceptance UI;
- hashed acceptance records;
- world-preservation and good-faith session rules;
- audited compliance/remediation state machine;
- non-punitive local hydration notices and achievements.

**Exit criteria:** Acceptance is tied to exact canonical bytes; every state transition is audited; remediation restores entitlement without any OS-level punitive behavior.

### Milestone 5.5 — Distribution

Deliver:

- production Vercel website and authenticated portal;
- automated GitHub Release publishing;
- signed release manifests and pinned-key verification;
- cached latest-release download page;
- verified launcher updater integration.

**Exit criteria:** A signed tag produces tested release assets without a website edit; the site resolves them; tampered metadata or binaries are rejected; failed updates preserve the current installation.

## 18. Verification strategy

### 18.1 Required automated tests

- Onboarding state transitions, safe Back behavior, checkpoint resume, retries, and redaction.
- Supported, unsupported, corrupt, unreadable, insufficient-space, cancelled, and interrupted import cases.
- Pre-flight severity and launch-gating rules for every check.
- Treaty canonicalization, version/hash matching, acceptance idempotency, and state transitions.
- Achievement definition validation, event idempotency, progress, unlock uniqueness, outbox restart, and multi-device reconciliation.
- Toast queue ordering, placement, duration, reduced motion, scaling, and first-visible-frame timing.
- Signed lease and release-manifest valid, expired, wrong-device, wrong-channel, rollback, bad-signature, wrong-size, and wrong-digest cases.
- Website authorization and cached/stale release metadata behavior.
- Installer clean install, upgrade, repair, and uninstall on supported Windows versions.

### 18.2 Five-second deterministic test

The runtime achievement component MUST accept an injectable monotonic clock, scheduler, local store, and toast sink. A release-blocking test shall:

1. start with player control false and no local unlock;
2. advance to the first frame with player control true at `t=0`;
3. simulate unavailable networking;
4. advance the fake clock and render frames;
5. assert the first visible `FTEP-ACH-0001` toast frame is at or before `t=5.000 s`;
6. assert the toast remains visible for the configured unmistakable duration;
7. continue emitting controllable frames and assert no duplicate;
8. restart from persisted local state and assert no duplicate;
9. restore networking and assert one idempotent server unlock.

Production telemetry MAY record coarse local deadline success/failure, but MUST NOT transmit gameplay frames or input data.

### 18.3 Absolute acceptance test

On a supported clean Windows system, a non-technical user must be able to:

```text
download installer
-> install
-> log in
-> accept treaty
-> select supported user-owned game data
-> click Play Zelda
-> reach a genuinely controllable Zelda state
-> see "Ahh, Zelda" begin within five seconds
```

No shell, manual configuration, file copying, hash interpretation, certificate management, or update handwork is permitted. The toast must read:

```text
Ahh, Zelda

Hope you had fun playing.
Now onto Satisfactory.
```

If this journey works reliably, FTEP has fulfilled Article I and the Compliance Directorate may begin enforcing Article II.

## 19. Definition of done

A feature is not complete merely because a backend endpoint or GUI mock exists. It is complete when the normal-user flow works without terminal intervention, failures have a safe recovery path, sensitive details are hidden and redacted, required state persists across restart, offline behavior follows this specification, automated tests cover its critical invariants, and Windows packaging exercises the integrated result.

Upstream Shipwright changes MUST remain narrow, reviewed, and separable from launcher/control-plane code. The ridiculous premise is not permission to compromise maintainability, privacy, or software architecture.
