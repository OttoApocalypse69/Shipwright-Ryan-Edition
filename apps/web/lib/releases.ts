import { z } from "zod";

const githubRelease = z.object({
  tag_name: z.string(),
  html_url: z.string().url(),
  draft: z.boolean(),
  prerelease: z.boolean(),
  published_at: z.string().nullable(),
  assets: z.array(z.object({ name: z.string(), browser_download_url: z.string().url(), size: z.number().int().nonnegative() })),
});

export type ReleaseSummary = { version: string; url: string; publishedAt: string | null; assets: Array<{ name: string; url: string; size: number }> };

export function parseStableRelease(input: unknown): ReleaseSummary {
  const release = githubRelease.parse(input);
  if (release.draft || release.prerelease) throw new Error("release is not stable");
  const expected = new Set(["SRE-Setup-x64.exe", "SRE-Portable-x64.zip", "SHA256SUMS.txt", "release-manifest.json", "release-manifest.sig"]);
  const assets = release.assets.filter((asset) => expected.has(asset.name));
  return { version: release.tag_name, url: release.html_url, publishedAt: release.published_at, assets: assets.map((asset) => ({ name: asset.name, url: asset.browser_download_url, size: asset.size })) };
}

export async function latestStableRelease(): Promise<ReleaseSummary | null> {
  const repository = process.env.FTEP_GITHUB_REPOSITORY;
  if (!repository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) return null;
  const response = await fetch(`https://api.github.com/repos/${repository}/releases/latest`, { headers: { Accept: "application/vnd.github+json" }, next: { revalidate: 300 } });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`GitHub release lookup failed with ${response.status}`);
  return parseStableRelease(await response.json());
}
