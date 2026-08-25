"use client";

import { useEffect, useState } from "react";

type Device = { deviceId: string; label: string | null; publicKey: string; registeredAt: string; lastSeenAt: string | null; revokedAt: string | null; revokedReason: string | null };

export function DeviceList() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [message, setMessage] = useState("Loading devices…");
  const [busy, setBusy] = useState<string>();
  const load = async () => {
    const response = await fetch("/api/devices", { cache: "no-store" });
    const value = await response.json() as { devices?: Device[]; error?: string };
    if (!response.ok) throw new Error(value.error ?? "Device list unavailable.");
    setDevices(value.devices ?? []);
    setMessage(value.devices?.length ? "" : "No SRE installations are registered yet.");
  };
  useEffect(() => { const run = async () => { try { await load(); } catch (error: unknown) { setMessage(error instanceof Error ? error.message : "Device list unavailable."); } }; void run(); }, []);
  const revoke = async (deviceId: string) => {
    if (!window.confirm("Revoke this SRE installation? It will not delete local files or saves.")) return;
    setBusy(deviceId);
    try {
      const response = await fetch("/api/devices", { method: "DELETE", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ deviceId }) });
      const value = await response.json() as { error?: string };
      if (!response.ok) throw new Error(value.error ?? "Device revocation failed.");
      await load();
    } catch (error: unknown) { setMessage(error instanceof Error ? error.message : "Device revocation failed."); }
    finally { setBusy(undefined); }
  };
  return <div className="grid">{message && <p className="muted">{message}</p>}{devices.map((device) => <article className="card" key={device.deviceId}><span className="badge">{device.revokedAt ? "REVOKED" : "ACTIVE"}</span><h3>{device.label ?? "SRE installation"}</h3><p className="muted">{device.deviceId}</p><p className="muted">Public key {device.publicKey.slice(0, 12)}… · Registered {new Date(device.registeredAt).toLocaleString()}</p>{device.revokedAt ? <p className="muted">Revoked {new Date(device.revokedAt).toLocaleString()}{device.revokedReason ? ` · ${device.revokedReason}` : ""}</p> : <button className="button secondary" disabled={busy === device.deviceId} onClick={() => void revoke(device.deviceId)}>{busy === device.deviceId ? "Revoking…" : "Revoke device"}</button>}</article>)}</div>;
}
