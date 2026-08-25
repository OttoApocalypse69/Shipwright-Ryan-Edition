"use client";

import { useCallback, useEffect, useState } from "react";

export type Section = "users" | "games" | "runtimes" | "compatibility" | "treaties" | "compliance" | "releases" | "audit";
const endpoint: Record<Section, string> = { users: "/api/admin/users", games: "/api/compatibility", runtimes: "/api/compatibility", compatibility: "/api/admin/compatibility", treaties: "/api/admin/compliance", compliance: "/api/admin/compliance", releases: "/api/admin/releases", audit: "/api/admin/audit" };

export function AdminDataPanel({ section }: { section: Section }) {
  const [data, setData] = useState<unknown>();
  const [message, setMessage] = useState("Loading control-plane data…");
  const [email, setEmail] = useState("");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => { const response = await fetch(endpoint[section], { cache: "no-store" }); const value = await response.json() as { error?: string }; if (!response.ok) throw new Error(value.error ?? "Control-plane data unavailable."); setData(value); setMessage(""); }, [section]);
  useEffect(() => { const run = async () => { try { await load(); } catch (error: unknown) { setMessage(error instanceof Error ? error.message : "Control-plane data unavailable."); } }; void run(); }, [load]);
  const transition = async (action: "suspend" | "restore") => { setBusy(true); try { const response = await fetch("/api/admin/entitlements", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ action, userEmail: email, reason }) }); const value = await response.json() as { error?: string }; if (!response.ok) throw new Error(value.error ?? "Operation failed."); setMessage(`Entitlement ${action} completed.`); await load(); } catch (error: unknown) { setMessage(error instanceof Error ? error.message : "Operation failed."); } finally { setBusy(false); } };
  return <><div className="panel"><p className="muted">{message}</p>{(section === "compliance" || section === "treaties") && <div className="account-form"><label>Account email<input value={email} onChange={(event) => setEmail(event.target.value)} type="email" /></label><label>Audited reason<textarea value={reason} onChange={(event) => setReason(event.target.value)} maxLength={1000} /></label><div className="actions"><button className="button" disabled={busy || !email || reason.length < 3} onClick={() => void transition("suspend")}>Suspend future entitlement</button><button className="button secondary" disabled={busy || !email || reason.length < 3} onClick={() => void transition("restore")}>Restore entitlement</button></div></div>}</div>{data && <pre className="panel" style={{ overflowX: "auto", whiteSpace: "pre-wrap" }}>{JSON.stringify(data, null, 2)}</pre>}</>;
}
