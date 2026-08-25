# FTEP deployment

The web app is Vercel-ready but is not deployed by this repository state. Provision PostgreSQL 17+ with TLS, apply `database/migrations/0001_ftep_core.sql` through `0004_animal_crossing_achievement.sql` in lexical order, then configure `apps/web/.env.example` variables in the hosting platform.

Required production values include `AUTH_SECRET`, at least one OAuth/OIDC client/secret pair, `DATABASE_URL`, GitHub repository coordinates, `FTEP_SIGNING_KEY_ID`, an Ed25519 PKCS#8 DER lease private key encoded as base64, and the matching raw Ed25519 release public key in `FTEP_RELEASE_SIGNING_PUBLIC_KEY`. Set `FTEP_LEASE_TTL_SECS` deliberately; the default is 24 hours. OAuth callback URLs must match the deployed Auth.js callback routes. Use a non-owner PostgreSQL application role and a separately controlled migration role.

Before deploying:

```powershell
pnpm install --frozen-lockfile
pnpm --filter @ftep/web test
pnpm --filter @ftep/web lint
pnpm --filter @ftep/web build
```

Deploy through the connected Vercel project or standard Vercel CLI/CI. Verify `/`, `/compatibility`, `/dashboard`, OAuth sign-in/out, unauthenticated `/admin` redirect, admin role denial/allow, device registration ownership conflict, signed device request replay rejection, lease issuance/signature, session and achievement synchronization, entitlement suspension/restoration, scope-request review, and GitHub release discovery/manifest verification. Configure monitoring for authentication failures, lease issuance, entitlement suspension/restoration, compatibility edits, device signature failures, API errors, latency, and database saturation without logging tokens or sensitive payloads.

No live deployment was performed during local finalization because hosting credentials, OAuth applications, PostgreSQL, and production signing keys are external operator inputs.

For the development-only local account and PostgreSQL workflow, see
[`LOCAL_FTEP_DEVELOPMENT.md`](LOCAL_FTEP_DEVELOPMENT.md). It must not be used as
a production deployment recipe.
