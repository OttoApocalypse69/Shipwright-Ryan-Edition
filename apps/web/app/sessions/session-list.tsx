"use client";

import { useEffect, useState } from "react";

type Session = { sessionId: string; state: string; title: string; gameId: string; variantId: string; runtimeId: string; requestedAt: string; startedAt: string | null; endedAt: string | null; durationMs: number | null; launchResult: string; deviceLabel: string | null };

export function SessionList() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [message, setMessage] = useState("Loading sessions…");
  useEffect(() => { void fetch("/api/sessions?limit=100", { cache: "no-store" }).then(async (response) => { const value = await response.json() as { sessions?: Session[]; error?: string }; if (!response.ok) throw new Error(value.error ?? "Session history unavailable."); setSessions(value.sessions ?? []); setMessage(value.sessions?.length ? "" : "No synchronized sessions yet."); }).catch((error: unknown) => setMessage(error instanceof Error ? error.message : "Session history unavailable.")); }, []);
  return <div className="grid">{message && <p className="muted">{message}</p>}{sessions.map((session) => <article className="card" key={session.sessionId}><span className="badge">{session.state}</span><h3>{session.title ?? session.gameId}</h3><p className="muted">{session.variantId} · {session.runtimeId} · {session.launchResult}</p><p className="muted">Requested {new Date(session.requestedAt).toLocaleString()}{session.durationMs ? ` · ${Math.floor(session.durationMs / 60000)} minutes` : ""}</p></article>)}</div>;
}
