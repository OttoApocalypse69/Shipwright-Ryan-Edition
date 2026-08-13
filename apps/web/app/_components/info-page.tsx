export function InfoPage({ eyebrow, title, intro, children }: { eyebrow: string; title: string; intro: string; children: React.ReactNode }) {
  return <section className="shell section"><span className="eyebrow">{eyebrow}</span><h2>{title}</h2><p className="lede">{intro}</p><div className="grid">{children}</div></section>;
}
export function InfoCard({ title, children }: { title: string; children: React.ReactNode }) { return <article className="card"><h3>{title}</h3><div className="muted">{children}</div></article>; }
