import { SessionList } from "./session-list";

export const metadata = { title: "Sessions" };
export default function SessionsPage() { return <section className="shell section"><span className="eyebrow">Lifecycle records</span><h2>Sessions</h2><p className="lede">SRE synchronizes privacy-preserving session summaries: title, variant, runtime, timing, playable signal, and result. It never uploads game files, saves, or process inventories.</p><SessionList /></section>; }
