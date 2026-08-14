# Local FTEP development

This checkout includes a local-only FTEP configuration for desktop development.
It is not a production deployment and does not expose the lease signing key to
the browser or to a release build.

## Start the local services

Docker Desktop must be running. The local PostgreSQL container is named
`ftep-local-postgres` and keeps its data while stopped.

Start Docker Desktop once, then start the `ftep-local-postgres` container if it
is not already running. SRE starts the local FTEP web service automatically
when its development launcher opens. It selects an unused `127.0.0.1` port and
keeps that URL private to the launcher, so it does not take port 3000.

For a terminal-free local startup, double-click
`START SRE.exe` at the top level of this project. The helper is built from this checkout
and starts the development launcher plus its managed FTEP service. It does not
start or stop Docker/PostgreSQL; those remain the durable local database
service. Its startup log is `target\sre-local-launcher.log`.

Open SRE and choose **Connect / refresh FTEP**. The browser opens the exact
local sign-up/sign-in page for the managed service. Local accounts are
intentionally available only when
`FTEP_LOCAL_CREDENTIALS_ENABLED=true` and `NODE_ENV` is not `production`.

The ignored `apps/web/.env.local` contains the local database connection and
development lease-signing key. Do not commit or reuse it for a deployment.

## Connect SRE

Open SRE and choose **Connect / refresh FTEP** in the library sidebar. Sign in
(or sign up), accept the Accord, and choose
**Ratify and connect SRE**. SRE validates and caches the resulting device-bound
lease locally; it remains usable offline until its 24-hour expiry.

The desktop trust entry used for this configuration is marked
`developmentOnly`. Debug builds accept it; release builds reject it. A real
deployment needs a separately controlled signing key and a release trust-set
update.

## Shutdown behavior

Closing SRE exits the launcher and stops only the FTEP process tree that SRE
started, freeing its temporary loopback port automatically. SRE does not stop
Docker, PostgreSQL, or any application/process it did not start. Closing local
FTEP does not remove your local game files, SRE library, or already cached
lease.
