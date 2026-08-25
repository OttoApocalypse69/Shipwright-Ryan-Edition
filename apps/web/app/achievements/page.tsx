"use client";

import { useEffect, useState } from "react";

type Achievement = { id: string; title: string; description: string; category: string; score: number; locked: boolean; unlockedAt: string | null };
export default function AchievementsPage() {
  const [achievements, setAchievements] = useState<Achievement[]>([]);
  const [message, setMessage] = useState("Loading achievements…");
  useEffect(() => { void fetch("/api/achievements", { cache: "no-store" }).then(async (response) => { const value = await response.json() as { achievements?: Achievement[]; error?: string }; if (!response.ok) throw new Error(value.error ?? "Achievement state unavailable."); setAchievements(value.achievements ?? []); setMessage(value.achievements ? "" : "No achievement definitions available."); }).catch((error: unknown) => setMessage(error instanceof Error ? error.message : "Achievement state unavailable.")); }, []);
  const unlocked = achievements.filter((achievement) => !achievement.locked).length;
  return <section className="shell section"><span className="eyebrow">Offline first</span><h2>Achievements</h2><p className="lede">{unlocked} / {achievements.length || "—"} synchronized. SRE unlocks locally first and retries synchronization when FTEP is available.</p><div className="grid">{message && <p className="muted">{message}</p>}{achievements.map((achievement) => <article className="card" key={achievement.id}><span className="badge">{achievement.locked ? "LOCKED" : `UNLOCKED · ${achievement.category}`}</span><h3>{achievement.locked ? "Classified FICSIT objective" : achievement.title}</h3><p>{achievement.locked ? "Keep using SRE to discover this achievement." : achievement.description}</p>{achievement.unlockedAt && <small>Unlocked {new Date(achievement.unlockedAt).toLocaleString()}</small>}</article>)}</div></section>;
}
