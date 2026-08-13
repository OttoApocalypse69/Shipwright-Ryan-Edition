import { createPrivateKey, createPublicKey } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

const [destination] = process.argv.slice(2);
const privateKeyBase64 = process.env.FTEP_LEASE_SIGNING_PRIVATE_KEY;
const keyId = process.env.FTEP_SIGNING_KEY_ID;

if (!destination || !privateKeyBase64 || !keyId) {
  throw new Error("usage: FTEP_LEASE_SIGNING_PRIVATE_KEY=... FTEP_SIGNING_KEY_ID=... create-trusted-keyset.mjs <destination>");
}
if (!/^[A-Za-z0-9._-]{1,80}$/.test(keyId)) throw new Error("FTEP_SIGNING_KEY_ID is invalid");

const privateKey = createPrivateKey({ key: Buffer.from(privateKeyBase64, "base64"), format: "der", type: "pkcs8" });
if (privateKey.asymmetricKeyType !== "ed25519") throw new Error("FTEP lease signing key must be Ed25519");
const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
const keyset = {
  schemaVersion: 1,
  keys: [{ keyId, algorithm: "Ed25519", publicKey: spki.subarray(spki.length - 32).toString("base64url") }],
};
await mkdir(dirname(destination), { recursive: true });
await writeFile(destination, `${JSON.stringify(keyset, null, 2)}\n`, { mode: 0o644 });
console.log(`wrote launcher trust set for ${keyId}`);
