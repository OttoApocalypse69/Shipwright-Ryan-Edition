import { generateKeyPairSync, verify } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { configuredProviderIds, localCredentialsEnabled, roleForEmail } from "@/lib/auth-config";
import { sanitizeEventPayload } from "@/lib/achievement-service";
import { mayPerform, requireAdmin } from "@/lib/authorization";
import { compatibilityRows } from "@/lib/catalog";
import { authenticateLocalAccount, createLocalAccount, hashLocalPassword, isLocalDatabaseUnavailable, safeLocalReturnTo, verifyLocalPassword } from "@/lib/local-credentials";
import { issueLocalLease, ratifyLocalDevice, readLocalFtepState, registerLocalDevice, revokeLocalDevice } from "@/lib/local-ftep-state";
import { parseStableRelease } from "@/lib/releases";
import { signLease } from "@/lib/signing";
import { accord, accordDocumentHash } from "@/lib/accord";

describe("authentication and authorization", () => {
  test("detects configured OAuth/OIDC providers without accepting partial secrets", () => {
    expect(configuredProviderIds({ AUTH_GITHUB_ID: "id", AUTH_GITHUB_SECRET: "secret", AUTH_OIDC_ID: "id", AUTH_OIDC_SECRET: "secret", AUTH_OIDC_ISSUER: "https://id.example" })).toEqual(["github", "oidc"]);
    expect(configuredProviderIds({ AUTH_GITHUB_ID: "id" })).toEqual([]);
  });
  test("enables local passwords only outside production", () => {
    expect(localCredentialsEnabled({ FTEP_LOCAL_CREDENTIALS_ENABLED: "true", NODE_ENV: "development" })).toBe(true);
    expect(localCredentialsEnabled({ FTEP_LOCAL_CREDENTIALS_ENABLED: "true", NODE_ENV: "production" })).toBe(false);
    expect(localCredentialsEnabled({ FTEP_LOCAL_CREDENTIALS_ENABLED: "false", NODE_ENV: "development" })).toBe(false);
  });
  test("admin role matching is exact and authorization defaults closed", () => {
    expect(roleForEmail("teri@example.com", "teri@example.com,other@example.com")).toBe("admin");
    expect(roleForEmail("attacker@example.com", "teri@example.com")).toBe("user");
    expect(mayPerform("admin", "compatibility.write")).toBe(true);
    expect(() => requireAdmin("user")).toThrow("FTEP_ADMIN_REQUIRED");
  });
});

test("local account passwords use a salted verifier and safe local return paths", async () => {
  const password = "not-a-real-password";
  const hash = await hashLocalPassword(password);
  expect(hash).not.toContain(password);
  await expect(verifyLocalPassword(password, hash)).resolves.toBe(true);
  await expect(verifyLocalPassword("wrong-password", hash)).resolves.toBe(false);
  expect(safeLocalReturnTo("/connect?state=abc")).toBe("/connect?state=abc");
  expect(safeLocalReturnTo("https://unsafe.example")).toBe("/dashboard");
  expect(safeLocalReturnTo("//unsafe.example")).toBe("/dashboard");
});

test("local FTEP treats a stale PostgreSQL schema as unavailable during development", () => {
  expect(isLocalDatabaseUnavailable({ code: "42703" })).toBe(true);
  expect(isLocalDatabaseUnavailable({ code: "42P01" })).toBe(true);
  expect(isLocalDatabaseUnavailable({ code: "23505" })).toBe(false);
});

test("local accounts fall back to the per-user development store when PostgreSQL is unavailable", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ftep-local-accounts-"));
  const previousStore = process.env.FTEP_LOCAL_ACCOUNT_STORE;
  const previousDatabaseUrl = process.env.DATABASE_URL;
  process.env.FTEP_LOCAL_ACCOUNT_STORE = join(directory, "accounts.json");
  delete process.env.DATABASE_URL;
  try {
    const registration = await createLocalAccount({ email: "local@example.test", password: "development-password", displayName: "Local player" });
    expect(registration.conflict).toBe(false);
    await expect(createLocalAccount({ email: "local@example.test", password: "development-password", displayName: "Duplicate" })).resolves.toMatchObject({ conflict: true });
    await expect(authenticateLocalAccount({ email: "local@example.test", password: "development-password" })).resolves.toMatchObject({ email: "local@example.test", displayName: "Local player" });
    await expect(authenticateLocalAccount({ email: "local@example.test", password: "wrong-development-password" })).resolves.toBeNull();
  } finally {
    if (previousStore === undefined) delete process.env.FTEP_LOCAL_ACCOUNT_STORE;
    else process.env.FTEP_LOCAL_ACCOUNT_STORE = previousStore;
    if (previousDatabaseUrl === undefined) delete process.env.DATABASE_URL;
    else process.env.DATABASE_URL = previousDatabaseUrl;
    await rm(directory, { recursive: true, force: true });
  }
});

test("local FTEP control-plane state survives without PostgreSQL", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ftep-local-state-"));
  const previousStore = process.env.FTEP_LOCAL_ACCOUNT_STORE;
  const previousControlPlane = process.env.FTEP_LOCAL_CONTROL_PLANE_STORE;
  const previousDatabaseUrl = process.env.DATABASE_URL;
  const previousPrivateKey = process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY;
  const previousKeyId = process.env.FTEP_SIGNING_KEY_ID;
  const pair = generateKeyPairSync("ed25519");
  process.env.FTEP_LOCAL_ACCOUNT_STORE = join(directory, "accounts.json");
  process.env.FTEP_LOCAL_CONTROL_PLANE_STORE = join(directory, "control-plane.json");
  delete process.env.DATABASE_URL;
  process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY = pair.privateKey.export({ format: "der", type: "pkcs8" }).toString("base64");
  process.env.FTEP_SIGNING_KEY_ID = "test-local";
  try {
    const account = await createLocalAccount({ email: "player@example.test", password: "development-password", displayName: "Player" });
    const deviceId = crypto.randomUUID();
    const publicKey = pair.publicKey.export({ format: "der", type: "spki" }).subarray(-32).toString("base64url");
    await registerLocalDevice({ email: account.account!.email, deviceId, publicKey, label: "SRE desktop" });
    await ratifyLocalDevice({ email: account.account!.email, deviceId, version: "3.0.0", documentHash: "hash" });
    const lease = await issueLocalLease({ email: account.account!.email, deviceId });
    const state = await readLocalFtepState();
    expect(state.devices).toHaveLength(1);
    expect(state.ratifications).toHaveLength(1);
    expect(state.entitlements).toEqual([expect.objectContaining({ key: "ftep.library.nintendo", state: "ACTIVE" })]);
    expect(state.leases).toHaveLength(1);
    expect(lease.keyId).toBe("test-local");
    await expect(revokeLocalDevice({ email: account.account!.email, deviceId })).resolves.toEqual({ deviceId, revoked: true });
    await expect(issueLocalLease({ email: account.account!.email, deviceId })).rejects.toThrow("No active entitlement");
    await expect(readLocalFtepState()).resolves.toMatchObject({ devices: [expect.objectContaining({ deviceId, revokedAtUnixMs: expect.any(Number) })] });
  } finally {
    if (previousStore === undefined) delete process.env.FTEP_LOCAL_ACCOUNT_STORE;
    else process.env.FTEP_LOCAL_ACCOUNT_STORE = previousStore;
    if (previousControlPlane === undefined) delete process.env.FTEP_LOCAL_CONTROL_PLANE_STORE;
    else process.env.FTEP_LOCAL_CONTROL_PLANE_STORE = previousControlPlane;
    if (previousDatabaseUrl === undefined) delete process.env.DATABASE_URL;
    else process.env.DATABASE_URL = previousDatabaseUrl;
    if (previousPrivateKey === undefined) delete process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY;
    else process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY = previousPrivateKey;
    if (previousKeyId === undefined) delete process.env.FTEP_SIGNING_KEY_ID;
    else process.env.FTEP_SIGNING_KEY_ID = previousKeyId;
    await rm(directory, { recursive: true, force: true });
  }
});

describe("public APIs", () => {
  test("achievement diagnostics stay inside the privacy boundary", () => {
    expect(sanitizeEventPayload({ gameId: "zelda-oot", metrics: { activeMinutes: 30 } })).toEqual({ gameId: "zelda-oot", metrics: { activeMinutes: 30 } });
    expect(() => sanitizeEventPayload({ metrics: { savePath: "C:\\Users\\player" } })).toThrow("FTEP_EVENT_PAYLOAD_NOT_ALLOWED");
    expect(() => sanitizeEventPayload({ data: "x".repeat(4097) })).toThrow("FTEP_EVENT_TOO_LARGE");
  });
  test("compatibility is represented per game variant and runtime", () => {
    const rows = compatibilityRows();
    expect(rows.some((row) => row.gameId === "zelda-botw" && row.runtimeId === "cemu-compatible" && row.status === "SUPPORTED")).toBe(true);
    expect(rows.filter((row) => ["zelda-totk", "zelda-echoes-of-wisdom", "animal-crossing-new-horizons"].includes(row.gameId)).every((row) => row.runtimeId === "switch-runtime")).toBe(true);
  });
  test("release lookup excludes unexpected assets and prereleases", () => {
    const release = parseStableRelease({ tag_name: "v0.1.0", html_url: "https://github.com/example/sre/releases/tag/v0.1.0", draft: false, prerelease: false, published_at: "2026-08-13T00:00:00Z", assets: [{ name: "SRE-Setup-x64.exe", browser_download_url: "https://github.com/example/sre/releases/download/v0.1.0/SRE-Setup-x64.exe", size: 10 }, { name: "unsafe.cmd", browser_download_url: "https://example.com/unsafe.cmd", size: 2 }] });
    expect(release.assets.map((asset) => asset.name)).toEqual(["SRE-Setup-x64.exe"]);
    expect(() => parseStableRelease({ tag_name: "v0.1.0", html_url: "https://github.com/example/sre/releases/tag/v0.1.0", draft: false, prerelease: false, published_at: null, assets: [{ name: "SRE-Setup-x64.exe", browser_download_url: "https://github.com/other/repo/releases/download/v0.1.0/SRE-Setup-x64.exe", size: 10 }] })).toThrow();
    expect(() => parseStableRelease({ tag_name: "v0.2.0-beta", html_url: "https://github.com/example/sre", draft: false, prerelease: true, published_at: null, assets: [] })).toThrow();
  });
});

test("server-side Ed25519 lease signer emits a verifiable payload", () => {
  const pair = generateKeyPairSync("ed25519");
  const privateKey = pair.privateKey.export({ format: "der", type: "pkcs8" }).toString("base64");
  const lease = { leaseId: crypto.randomUUID(), subjectId: crypto.randomUUID(), deviceId: crypto.randomUUID(), entitlements: ["ftep.library.nintendo"], issuedAtUnixSecs: 1, notBeforeUnixSecs: 1, expiresAtUnixSecs: 2 };
  const signed = signLease(lease, privateKey, "test-key");
  expect(verify(null, Buffer.from(signed.payload, "base64url"), pair.publicKey, Buffer.from(signed.signature, "base64url"))).toBe(true);
  expect(signed.keyId).toBe("test-key");
});

test("canonical Accord identity is stable for the launcher callback", () => {
  expect(accord.version).toBe("3.0.0");
  expect(accord.articles).toHaveLength(17);
  expect(accordDocumentHash).toMatch(/^[a-f0-9]{64}$/);
});
