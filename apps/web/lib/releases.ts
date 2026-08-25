import { createPublicKey, verify } from "node:crypto";
import { z } from "zod";

const githubRelease = z.object({
  tag_name: z.string(),
  html_url: z.string().url(),
  draft: z.boolean(),
  prerelease: z.boolean(),
  published_at: z.string().nullable(),
  assets: z.array(z.object({ name: z.string(), browser_download_url: z.string().url(), size: z.number().int().nonnegative() })),
});

const manifestSchema = z.object({
  schemaVersion: z.literal(1),
  product: z.literal("SRE"),
  version: z.string(),
  channel: z.enum(["stable", "beta", "nightly"]),
  publishedAt: z.string().datetime({ offset: true }),
  assets: z.array(z.object({ name: z.string(), url: z.string().url(), sha256: z.string().regex(/^[a-f0-9]{64}$/), size: z.number().int().nonnegative() })),
});

const expectedAssets = new Set(["SRE-Setup-x64.exe", "SRE-Portable-x64.zip", "SHA256SUMS.txt", "release-manifest.json", "release-manifest.sig"]);
const signedPayloadAssets = new Set(["SRE-Setup-x64.exe", "SRE-Portable-x64.zip", "SHA256SUMS.txt"]);
const publicKeyPrefix = Buffer.from("302a300506032b6570032100", "hex");

export type ReleaseSummary = {
  version: string;
  url: string;
  publishedAt: string | null;
  assets: Array<{ name: string; url: string; size: number }>;
  verified: boolean;
  verificationReason: string;
  manifest?: z.infer<typeof manifestSchema>;
};

function githubUrl(value: string): boolean {
  const url = new URL(value);
  return url.protocol === "https:" && url.hostname === "github.com";
}

function releaseCoordinates(value: string): { repository: string; tag: string } | null {
  try {
    const url = new URL(value);
    if (!githubUrl(value)) return null;
    const segments = url.pathname.split("/").filter(Boolean).map((segment) => decodeURIComponent(segment));
    if (segments.length !== 5 || segments[2] !== "releases" || segments[3] !== "tag") return null;
    if (!/^[A-Za-z0-9_.-]+$/.test(segments[0]) || !/^[A-Za-z0-9_.-]+$/.test(segments[1]) || !segments[4]) return null;
    return { repository: `${segments[0]}/${segments[1]}`, tag: segments[4] };
  } catch {
    return null;
  }
}

function githubReleaseAssetUrl(value: string, coordinates: { repository: string; tag: string }, assetName: string): boolean {
  try {
    const url = new URL(value);
    if (!githubUrl(value)) return false;
    const segments = url.pathname.split("/").filter(Boolean).map((segment) => decodeURIComponent(segment));
    const [owner, repository] = coordinates.repository.split("/");
    return segments.length === 6
      && segments[0] === owner
      && segments[1] === repository
      && segments[2] === "releases"
      && segments[3] === "download"
      && segments[4] === coordinates.tag
      && segments[5] === assetName;
  } catch {
    return false;
  }
}

export function parseStableRelease(input: unknown): ReleaseSummary {
  const release = githubRelease.parse(input);
  if (release.draft || release.prerelease) throw new Error("release is not stable");
  const coordinates = releaseCoordinates(release.html_url);
  if (!coordinates) throw new Error("release URL is not an approved GitHub release URL");
  const candidateAssets = release.assets.filter((asset) => expectedAssets.has(asset.name));
  if (candidateAssets.some((asset) => !githubReleaseAssetUrl(asset.browser_download_url, coordinates, asset.name))) throw new Error("release asset URL is not an approved GitHub release asset URL");
  const assets = candidateAssets;
  return { version: release.tag_name, url: release.html_url, publishedAt: release.published_at, assets: assets.map((asset) => ({ name: asset.name, url: asset.browser_download_url, size: asset.size })), verified: false, verificationReason: "Signed release manifest has not been checked." };
}

async function verifyManifest(release: ReleaseSummary): Promise<Pick<ReleaseSummary, "verified" | "verificationReason" | "manifest">> {
  const manifestAsset = release.assets.find((asset) => asset.name === "release-manifest.json");
  const signatureAsset = release.assets.find((asset) => asset.name === "release-manifest.sig");
  const publicKey = process.env.FTEP_RELEASE_SIGNING_PUBLIC_KEY;
  if (!manifestAsset || !signatureAsset) return { verified: false, verificationReason: "The release did not publish both manifest files." };
  if (!publicKey) return { verified: false, verificationReason: "FTEP_RELEASE_SIGNING_PUBLIC_KEY is not configured." };
  try {
    const [manifestResponse, signatureResponse] = await Promise.all([
      fetch(manifestAsset.url, { next: { revalidate: 900 } }),
      fetch(signatureAsset.url, { next: { revalidate: 900 } }),
    ]);
    if (!manifestResponse.ok || !signatureResponse.ok) return { verified: false, verificationReason: "The release manifest could not be downloaded." };
    const bytes = Buffer.from(await manifestResponse.arrayBuffer());
    const signature = Buffer.from((await signatureResponse.text()).trim(), "base64");
    const rawKey = Buffer.from(publicKey, "base64url");
    if (rawKey.length !== 32 || signature.length !== 64) return { verified: false, verificationReason: "The release signing material is malformed." };
    const key = createPublicKey({ key: Buffer.concat([publicKeyPrefix, rawKey]), format: "der", type: "spki" });
    if (!verify(null, bytes, key, signature)) return { verified: false, verificationReason: "The release manifest signature is invalid." };
    const manifest = manifestSchema.parse(JSON.parse(bytes.toString("utf8")));
    if (manifest.version !== release.version || manifest.channel !== "stable") return { verified: false, verificationReason: "The signed manifest identity does not match the stable release." };
    const coordinates = releaseCoordinates(release.url);
    if (!coordinates) return { verified: false, verificationReason: "The stable release URL is not an approved GitHub release." };
    const publishedAssets = new Map(release.assets.map((asset) => [asset.name, asset]));
    const signedAssetNames = new Set(manifest.assets.map((asset) => asset.name));
    if (signedAssetNames.size !== signedPayloadAssets.size || [...signedPayloadAssets].some((name) => !signedAssetNames.has(name))) {
      return { verified: false, verificationReason: "The signed manifest does not cover the complete release payload." };
    }
    for (const asset of manifest.assets) {
      const published = publishedAssets.get(asset.name);
      if (!published || published.size !== asset.size || !githubReleaseAssetUrl(asset.url, coordinates, asset.name) || asset.url !== published.url) return { verified: false, verificationReason: "The signed manifest does not match the published assets." };
    }
    return { verified: true, verificationReason: "Signature, channel, version, URLs, and asset sizes verified.", manifest };
  } catch {
    return { verified: false, verificationReason: "The signed release manifest is invalid." };
  }
}

export async function latestStableRelease(): Promise<ReleaseSummary | null> {
  const repository = process.env.FTEP_GITHUB_REPOSITORY;
  if (!repository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) return null;
  const response = await fetch(`https://api.github.com/repos/${repository}/releases/latest`, { headers: { Accept: "application/vnd.github+json" }, next: { revalidate: 900 } });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`GitHub release lookup failed with ${response.status}`);
  const release = parseStableRelease(await response.json());
  return { ...release, ...(await verifyManifest(release)) };
}
