import Link from "next/link";
import { catalog } from "@/lib/catalog";

export const metadata = { title: "Games" };
export default function GamesPage() { return <section className="shell section"><span className="eyebrow">Data-driven catalog</span><h2>Games</h2><p className="lede">Every title uses the same game, variant, installation, and runtime-candidate model.</p><div className="grid">{catalog.games.map((game) => <Link className="card" href={`/games/${game.id}`} key={game.id}><div className="cover" style={{ color: game.cover.accentColor, background: game.cover.backgroundColor }}>{game.cover.titleMark}</div><h3>{game.title}</h3><p>{game.description}</p><span className={`badge ${game.variants[0].runtimeCandidates[0].compatibility}`}>{game.variants[0].runtimeCandidates[0].compatibility}</span></Link>)}</div></section>; }
