import { latestStableRelease } from "@/lib/releases";

export const metadata = { title: "Download" };
export default async function DownloadPage() {
  const release = await latestStableRelease().catch(() => null);
  const installer = release?.assets.find((asset) => asset.name === "SRE-Setup-x64.exe");
  return <section className="shell section"><span className="eyebrow">Windows x64</span><h2>Download SRE</h2><p className="lede">Stable is the default channel. Beta and nightly releases are for compatibility testing.</p><article className="panel"><h3>{release ? `Latest stable: ${release.version}` : "Release feed not configured"}</h3><p className="muted">Binaries are hosted by GitHub Releases and discovered automatically. Every release includes a signed manifest and SHA-256 checksums.</p>{installer ? <a className="button" href={installer.url}>SRE-Setup-x64.exe</a> : <span className="badge">NO PUBLISHED RC</span>}<div className="metric"><span>Verification</span><strong>{release?.verified ? "SIGNED MANIFEST VERIFIED" : release?.verificationReason ?? "No release metadata"}</strong></div><div className="metric"><span>Expected assets</span><strong>SRE-Setup · Portable · checksums · signed manifest</strong></div><div className="metric"><span>Channels</span><strong>stable / beta / nightly</strong></div></article></section>;
}
