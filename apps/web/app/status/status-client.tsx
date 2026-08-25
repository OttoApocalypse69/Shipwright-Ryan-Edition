"use client";

import { useEffect, useState } from "react";

type Status = { checkedAt: string; services: Record<string, string> };
export function StatusClient() {
  const [status, setStatus] = useState<Status>();
  const [message, setMessage] = useState("Checking service health…");
  useEffect(() => { void fetch("/api/status", { cache: "no-store" }).then(async (response) => { const value = await response.json() as Status; if (!response.ok) throw new Error("Status unavailable."); setStatus(value); setMessage(""); }).catch((error: unknown) => setMessage(error instanceof Error ? error.message : "Status unavailable.")); }, []);
  return <div className="grid">{message && <p className="muted">{message}</p>}{status && Object.entries(status.services).map(([service, value]) => <article className="card" key={service}><span className={`badge ${value === "healthy" || value === "configured" ? "SUPPORTED" : "DEGRADED"}`}>{value.toUpperCase()}</span><h3>{service.replaceAll(/([A-Z])/g, " $1")}</h3><p className="muted">Measured {new Date(status.checkedAt).toLocaleString()}</p></article>)}</div>;
}
