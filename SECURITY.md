# Security policy

Report suspected vulnerabilities privately to the repository maintainers. Do not open a public issue containing exploit details, private keys, OAuth tokens, personal data, proprietary game data, or a working attack against deployed infrastructure.

## In scope

Current SRE launcher, FTEP web/API, entitlement and updater signature verification, device-key storage, release tooling, runtime argument/path handling, diagnostics redaction, and PostgreSQL authorization boundaries are security-sensitive.

## Supported code

Only the current default branch and the latest published stable release are intended to receive security fixes. This repository snapshot is a release candidate, not evidence that a public deployment exists.

## Principles

- Private signing keys are server/release secrets and must never enter source, logs, artifacts, or the launcher.
- Game data remains local and must not be uploaded or logged.
- Revocation and policy enforcement are non-destructive.
- External runtimes are untrusted third-party software and are never downloaded by SRE.
- Findings should include affected commit, reproduction, impact, and suggested containment where possible.
