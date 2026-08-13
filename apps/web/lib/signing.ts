import { createPrivateKey, sign } from "node:crypto";

export type LeasePayload = {
  leaseId: string; subjectId: string; deviceId: string; entitlements: string[];
  issuedAtUnixSecs: number; notBeforeUnixSecs: number; expiresAtUnixSecs: number;
};

export function signLease(payload: LeasePayload, privateKeyBase64 = process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY, keyId = process.env.FTEP_SIGNING_KEY_ID): { payload: string; signature: string; keyId: string } {
  if (!privateKeyBase64 || !keyId) throw new Error("FTEP lease signing is not configured");
  const key = createPrivateKey({ key: Buffer.from(privateKeyBase64, "base64"), format: "der", type: "pkcs8" });
  if (key.asymmetricKeyType !== "ed25519") throw new Error("FTEP lease key must be Ed25519");
  const bytes = Buffer.from(JSON.stringify(payload));
  return { payload: bytes.toString("base64url"), signature: sign(null, bytes, key).toString("base64url"), keyId };
}

export function publicKeyRawBase64(privateKeyBase64 = process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY): string {
  if (!privateKeyBase64) throw new Error("FTEP lease signing is not configured");
  const key = createPrivateKey({ key: Buffer.from(privateKeyBase64, "base64"), format: "der", type: "pkcs8" });
  const spki = key.export({ format: "der", type: "spki" });
  return spki.subarray(spki.length - 32).toString("base64url");
}
