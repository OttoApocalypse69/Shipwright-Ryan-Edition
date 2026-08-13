# Security architecture and release checklist

The security policy is at the repository root. Primary controls are Ed25519 lease/update signatures, installer-bundled public trust, local random device keys, OAuth/OIDC through Auth.js, JWT session limits, secure/httpOnly/same-site cookies in production, admin role enforcement, parameterized SQL, audit events, CSP nonces, restrictive browser headers, exact release asset allowlists, HTTPS GitHub update URLs, SHA-256/size verification, atomic local writes, direct process arguments, and diagnostics redaction.

Release preflight:

1. Run `pnpm install --frozen-lockfile` and `pnpm check` on the exact commit.
2. Run `pnpm audit --prod` and `cargo audit`; triage results without suppressing them silently.
3. Run the standard Codex Security scan and resolve or document every validated finding.
4. Confirm production OAuth redirect URIs and a strong `AUTH_SECRET`.
5. Confirm least-privilege PostgreSQL credentials, TLS, backups, and migrations.
6. Generate the launcher trust set from the production lease key; inspect it contains only public key material.
7. Build/package in CI with protected Ed25519 lease/release keys. Never use fake or test signatures for publication.
8. Verify checksums/signature, install/launch/upgrade/uninstall lifecycle, and absence of proprietary content.
9. Review third-party license evidence and block redistribution while any bundled component remains unresolved.

Known architectural limits: a user-controlled desktop is not tamper-proof; newly issued server revocation cannot reach a genuinely offline client until reconnect or lease expiry; and public redistribution is currently blocked by incomplete inherited license evidence.
