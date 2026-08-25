# Local FTEP development

This checkout includes a local-only FTEP configuration for desktop development.
It is not a production deployment and does not expose the lease signing key to
the browser or to a release build.

## Start the local services

Docker Desktop is optional for local development. If the local PostgreSQL
container is available, FTEP uses it as the primary control-plane store. If it
is stopped or has not been created yet, local development automatically uses a
durable file-backed control-plane store instead, so account/device/Accord/lease
state is not lost.

The optional local PostgreSQL container is named `ftep-local-postgres` and
keeps its data while stopped.

If you want PostgreSQL-backed state, SRE automatically starts Docker Desktop
when it is not already running, then starts the stopped
`ftep-local-postgres` container. An already-running container is left alone.
SRE starts the local FTEP web service automatically when its development
launcher opens. It selects an unused `127.0.0.1` port and keeps that URL private
to the launcher, so it does not take port 3000.

For a terminal-free local startup, double-click
`START SRE.exe` at the top level of this project. The helper is built from this checkout
and starts the development launcher plus its managed FTEP service. If SRE had
to start Docker or PostgreSQL, it shuts those services down when SRE closes.
The startup log is `target\sre-local-launcher.log`; the FTEP log also records
Docker startup decisions.

`START SRE.exe` is safe to click more than once. It keeps an OS-level lock for
the lifetime of the running helper, so a second click exits successfully and
leaves the existing SRE window and local services untouched. If a forced close
left the Vite frontend listening on port 1420 without an SRE window, the next
start reuses that frontend and opens a fresh SRE instance rather than reporting
the port as a fatal error.

Open SRE and choose **Connect / refresh FTEP**. The browser opens the exact
local sign-up/sign-in page for the managed service. After the first successful
local sign-in, the browser session is retained across SRE restarts. Local accounts are
intentionally available only when
`FTEP_LOCAL_CREDENTIALS_ENABLED=true` and `NODE_ENV` is not `production`.

The ignored `apps/web/.env.local` contains the local database connection and
development lease-signing key. Do not commit or reuse it for a deployment.
The SRE-managed FTEP process passes this file's local-only variables explicitly
to its server process, including the signing key; the key is never sent to the
browser or to a release build.
When PostgreSQL is unavailable, sign-up/sign-in and the complete connection
flow transparently use these development-only stores:

- `%LOCALAPPDATA%\\Shipwright Ryan Edition\\ftep\\local-accounts.json` for
  salted scrypt password verifiers;
- `%LOCALAPPDATA%\\Shipwright Ryan Edition\\ftep\\local-control-plane.json`
  for device registration, Accord ratification, active local entitlements,
  signed lease history, and bounded audit events.

The paths can be overridden with `FTEP_LOCAL_ACCOUNT_STORE` and
`FTEP_LOCAL_CONTROL_PLANE_STORE`. This fallback is available only for local
development accounts, never in production.

If the container was created by an older checkout, the same fallback also covers
the brief period before migrations `0003_ftep_production_control_plane.sql` and
`0004_animal_crossing_achievement.sql` are applied. To use PostgreSQL-backed
state immediately, apply every file in `database/migrations` in lexical order;
the current checkout's startup and CI documentation expect all four migrations.

## Connect SRE

Open SRE and choose **Connect / refresh FTEP** in the library sidebar. Sign in
(or sign up), accept the Accord, and choose
**Ratify and connect SRE**. SRE validates and caches the resulting device-bound
lease locally; the local development lease remains usable offline for 30 days
by default. Production leases remain controlled by the deployed FTEP policy.

The desktop trust entry used for this configuration is marked
`developmentOnly`. Debug builds accept it; release builds reject it. A real
deployment needs a separately controlled signing key and a release trust-set
update.

If an existing local device UUID is already owned by another account, FTEP
does not overwrite that registration. The browser returns a structured
ownership-conflict callback, and SRE rotates its local random identity once,
removes the expired cached lease, and retries registration. This keeps account
boundaries intact while making account switching and reconnecting recoverable.

## Shutdown behavior

Closing SRE exits the launcher and stops the FTEP process tree and PostgreSQL
container that SRE started, freeing its temporary loopback port automatically.
If SRE also started Docker Desktop, it stops Docker Desktop only when no other
container is running. A container or Docker Desktop that was already running
before SRE opened is left alone. Closing local FTEP does not remove your local
game files, SRE library, database volume, or already cached lease.
