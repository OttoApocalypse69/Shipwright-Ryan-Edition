import { StatusClient } from "./status-client";

export const metadata = { title: "Status" };
export default function StatusPage() { return <section className="shell section"><span className="eyebrow">Service status</span><h2>Status</h2><p className="lede">FTEP reports measured control-plane, authentication, signing, achievement, and release metadata state without exposing internal hosts or personal data.</p><StatusClient /></section>; }
