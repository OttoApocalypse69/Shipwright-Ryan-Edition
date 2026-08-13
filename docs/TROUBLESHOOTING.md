# Troubleshooting

Run SRE **Diagnostics** and export the sanitized report, or use:

```powershell
cargo run -p sre-diagnostics --bin sre -- doctor --json
```

Common cases:

- **Signing key is not trusted:** the lease key is absent from this build's bundled trust set. Install a newer official SRE build; do not paste a key into the launcher.
- **No/expired entitlement:** reconnect through the FTEP dashboard and obtain a current device-bound lease. Offline launches work only until cached expiry.
- **Game source rejected:** confirm the correct title/variant and lawful complete source layout. SRE does not locate missing content.
- **Runtime missing/wrong version:** select the executable you installed and review provider diagnostics. SRE does not download external runtimes.
- **No playable achievement:** the provider may have no signal or only an approximate delayed signal. Check session/doctor output before changing compatibility claims.
- **Web says credentials required:** set `AUTH_SECRET` and a supported OAuth/OIDC provider. Database and signing routes need their own production variables.
- **Installer cannot build:** prepare Shipwright resources with the Windows C++/SDK environment first; see the launcher README.

Diagnostic exports deliberately omit raw paths and secrets. Include the SRE version, diagnostic request ID, catalog game/variant/runtime IDs, and reproducible steps when reporting a problem.
