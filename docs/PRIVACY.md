# Privacy

SRE stores library paths, local configuration, a random device UUID/keypair, signed lease, sessions, achievements, and diagnostic state on the user's machine. Game data, saves, console keys, firmware, and runtime files are not uploaded by default.

When connected, FTEP receives the authenticated account identity supplied by the OAuth provider, the random device UUID and public key, Accord/compliance records, entitlement/lease metadata, session summaries, achievement synchronization, and security/audit events necessary to operate the service. It does not require a hardware fingerprint.

Sanitized diagnostics replace sensitive paths with hashes and exclude private device keys, OAuth tokens, signed-session cookies, signing private keys, and game content. Operators must define production retention/deletion periods and publish jurisdiction-specific policy before deployment. Users can remove local application data after uninstall; server data deletion requires the deployed account/privacy workflow.
