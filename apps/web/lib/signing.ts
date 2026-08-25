import { createPrivateKey, createPublicKey, sign } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

export type LeasePayload = {
  leaseId: string; subjectId: string; deviceId: string; entitlements: string[];
  issuedAtUnixSecs: number; notBeforeUnixSecs: number; expiresAtUnixSecs: number;
};

function localEnvValue(name: string): string | undefined {
  if (process.env.NODE_ENV === "production") return undefined;
  const paths = [join(process.cwd(), ".env.local"), join(process.cwd(), "apps/web/.env.local")];
  for (const path of paths) {
    try {
      const line = readFileSync(path, "utf8").split(/\r?\n/).find((value) => value.trim().startsWith(`${name}=`));
      if (!line) continue;
      const value = line.slice(name.length + 1).trim();
      return value.replace(/^(['"])(.*)\1$/, "$2");
    } catch {
      // The production path never reads local files; a missing development
      // file simply falls through to the normal environment lookup.
    }
  }
  return undefined;
}

function configuredValue(name: string): string | undefined {
  return process.env[name] || localEnvValue(name);
}

export function signLease(payload: LeasePayload, privateKeyBase64?: string, keyId?: string): { payload: string; signature: string; keyId: string } {
  const configuredPrivateKey = privateKeyBase64 ?? configuredValue("FTEP_LEASE_SIGNING_PRIVATE_KEY");
  const configuredKeyId = keyId ?? configuredValue("FTEP_SIGNING_KEY_ID");
  if (!configuredPrivateKey || !configuredKeyId) throw new Error("FTEP lease signing is not configured");
  const key = createPrivateKey({ key: Buffer.from(configuredPrivateKey, "base64"), format: "der", type: "pkcs8" });
  if (key.asymmetricKeyType !== "ed25519") throw new Error("FTEP lease key must be Ed25519");
  const bytes = Buffer.from(JSON.stringify(payload));
  return { payload: bytes.toString("base64url"), signature: sign(null, bytes, key).toString("base64url"), keyId: configuredKeyId };
}

export function publicKeyRawBase64(privateKeyBase64?: string): string {
  const configuredPrivateKey = privateKeyBase64 ?? configuredValue("FTEP_LEASE_SIGNING_PRIVATE_KEY");
  if (!configuredPrivateKey) throw new Error("FTEP lease signing is not configured");
  const key = createPrivateKey({ key: Buffer.from(configuredPrivateKey, "base64"), format: "der", type: "pkcs8" });
  const spki = createPublicKey(key).export({ format: "der", type: "spki" });
  return spki.subarray(spki.length - 32).toString("base64url");
}
