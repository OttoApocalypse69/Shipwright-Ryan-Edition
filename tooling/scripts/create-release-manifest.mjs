import { createHash, createPrivateKey, sign } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [directory, version, channel, baseUrl] = process.argv.slice(2);
if (!directory || !version || !["stable", "beta", "nightly"].includes(channel) || !baseUrl) {
  throw new Error("usage: create-release-manifest.mjs <directory> <version> <stable|beta|nightly> <release-base-url>");
}
if (!process.env.FTEP_RELEASE_SIGNING_PRIVATE_KEY) throw new Error("FTEP_RELEASE_SIGNING_PRIVATE_KEY is required");
const names = ["SRE-Setup-x64.exe", "SRE-Portable-x64.zip", "SHA256SUMS.txt"];
const assets = [];
for (const name of names) {
  const path = join(directory, name); const bytes = await readFile(path); const metadata = await stat(path);
  assets.push({ name, url: `${baseUrl.replace(/\/$/, "")}/${encodeURIComponent(name)}`, sha256: createHash("sha256").update(bytes).digest("hex"), size: metadata.size });
}
const manifest = { schemaVersion: 1, product: "SRE", version, channel, publishedAt: new Date().toISOString(), assets };
const bytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
const privateKey = createPrivateKey({ key: Buffer.from(process.env.FTEP_RELEASE_SIGNING_PRIVATE_KEY, "base64"), format: "der", type: "pkcs8" });
if (privateKey.asymmetricKeyType !== "ed25519") throw new Error("release signing key must be Ed25519");
await writeFile(join(directory, "release-manifest.json"), bytes);
await writeFile(join(directory, "release-manifest.sig"), `${sign(null, bytes, privateKey).toString("base64")}\n`);
console.log(`signed SRE ${version} ${channel} manifest with ${assets.length} assets`);
