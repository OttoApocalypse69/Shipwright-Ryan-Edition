import Link from "next/link";
import { catalog } from "@/lib/catalog";

export default function Home() {
  return <>
    <section className="shell hero"><div><span className="eyebrow">Super Runtime Environment</span><h1>SRE</h1><p className="lede">One library.<br />Multiple runtimes.<br />Far too much infrastructure.</p><p className="muted">Powered by the FICSIT Treaty Enforcement Platform.</p><div className="actions"><Link className="button" href="/download">Download for Windows</Link><Link className="button secondary" href="/compatibility">View compatibility</Link><Link className="button secondary" href="/treaty">View the Accord</Link></div></div>
      <aside className="panel"><span className="eyebrow">Release candidate</span><h3>Local-first runtime orchestration</h3><p className="muted">Your game data stays yours. SRE coordinates user-managed runtimes and locally cached authorization.</p><div className="metric"><span>Catalog titles</span><strong>{catalog.games.length}</strong></div><div className="metric"><span>Primary platforms</span><strong>Windows x64</strong></div><div className="metric"><span>Telemetry</span><strong>Opt-in</strong></div></aside>
    </section>
    <section className="shell section"><span className="eyebrow">The useful part</span><h2>A library, not an emulator bundle.</h2><div className="grid"><article className="card"><h3>Bring your own data</h3><p>SRE validates paths you select. It does not download Nintendo material or circumvention tooling.</p></article><article className="card"><h3>Honest compatibility</h3><p>Support is tracked per game, variant, runtime, and version range. Experimental means experimental.</p></article><article className="card"><h3>Works offline</h3><p>Achievements and cached short-lived entitlement leases are verified locally before later synchronization.</p></article></div></section>
  </>;
}
