import { gameById, catalog } from "@/lib/catalog";
import type { Metadata } from "next";
import { notFound } from "next/navigation";

type Props = { params: Promise<{ id: string }> };
export function generateStaticParams() { return catalog.games.map((game) => ({ id: game.id })); }
export async function generateMetadata({ params }: Props): Promise<Metadata> { const { id } = await params; const game = gameById(id); return { title: game?.title ?? "Game" }; }
export default async function GamePage({ params }: Props) {
  const { id } = await params; const game = gameById(id); if (!game) notFound();
  return <section className="shell section"><span className="eyebrow">{game.franchise}</span><h2>{game.title}</h2><p className="lede">{game.description}</p><div className="grid">{game.variants.map((variant) => <article className="card" key={variant.id}><h3>{variant.id.toUpperCase()}</h3><p>Original platform: {variant.originalPlatform.replaceAll("_", " ")}</p>{variant.runtimeCandidates.map((runtime) => <div className="metric" key={runtime.runtimeId}><span>{runtime.runtimeId}</span><span className={`badge ${runtime.compatibility}`}>{runtime.compatibility}</span></div>)}<h3>What you provide</h3>{variant.sourceRequirements.map((item) => <p key={item.id}>{item.required ? "Required" : "Optional"}: {item.description}</p>)}</article>)}</div></section>;
}
