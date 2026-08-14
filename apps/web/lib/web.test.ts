import { generateKeyPairSync, verify } from "node:crypto";
import { describe, expect, test } from "vitest";
import { configuredProviderIds, localCredentialsEnabled, roleForEmail } from "@/lib/auth-config";
import { mayPerform, requireAdmin } from "@/lib/authorization";
import { compatibilityRows } from "@/lib/catalog";
import { hashLocalPassword, safeLocalReturnTo, verifyLocalPassword } from "@/lib/local-credentials";
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

describe("public APIs", () => {
  test("compatibility is represented per game variant and runtime", () => {
    const rows = compatibilityRows();
    expect(rows.some((row) => row.gameId === "zelda-botw" && row.runtimeId === "cemu-compatible" && row.status === "EXPERIMENTAL")).toBe(true);
    expect(rows.filter((row) => ["zelda-totk", "zelda-echoes-of-wisdom", "animal-crossing-new-horizons"].includes(row.gameId)).every((row) => row.runtimeId === "switch-runtime")).toBe(true);
  });
  test("release lookup excludes unexpected assets and prereleases", () => {
    const release = parseStableRelease({ tag_name: "v0.1.0", html_url: "https://github.com/example/sre/releases/tag/v0.1.0", draft: false, prerelease: false, published_at: "2026-08-13T00:00:00Z", assets: [{ name: "SRE-Setup-x64.exe", browser_download_url: "https://github.com/example/sre/releases/download/v0.1.0/SRE-Setup-x64.exe", size: 10 }, { name: "unsafe.cmd", browser_download_url: "https://example.com/unsafe.cmd", size: 2 }] });
    expect(release.assets.map((asset) => asset.name)).toEqual(["SRE-Setup-x64.exe"]);
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
