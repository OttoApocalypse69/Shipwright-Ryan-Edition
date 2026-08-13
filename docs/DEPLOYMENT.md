# FTEP deployment

The web app is Vercel-ready but is not deployed by this repository state. Provision PostgreSQL 17+ with TLS, apply `database/migrations/*.sql` in lexical order, then configure `apps/web/.env.example` variables in the hosting platform.

Required production values include `AUTH_SECRET`, at least one OAuth/OIDC client/secret pair, `DATABASE_URL`, GitHub repository coordinates, `FTEP_SIGNING_KEY_ID`, and an Ed25519 PKCS#8 DER lease private key encoded as base64. OAuth callback URLs must match the deployed Auth.js callback routes. Use a non-owner PostgreSQL application role and a separately controlled migration role.

Before deploying:

```powershell
pnpm install --frozen-lockfile
pnpm --filter @ftep/web test
pnpm --filter @ftep/web lint
pnpm --filter @ftep/web build
```

Deploy through the connected Vercel project or standard Vercel CLI/CI. Verify `/`, `/compatibility`, `/dashboard`, OAuth sign-in/out, unauthenticated `/admin` redirect, admin role denial/allow, device registration ownership conflict, lease issuance/signature, and GitHub release discovery. Configure monitoring for authentication failures, lease issuance, entitlement suspension/restoration, compatibility edits, API errors, latency, and database saturation without logging tokens or sensitive payloads.

No live deployment was performed during local finalization because hosting credentials, OAuth applications, PostgreSQL, and production signing keys are external operator inputs.
