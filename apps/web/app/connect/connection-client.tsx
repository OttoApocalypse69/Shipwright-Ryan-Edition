"use client";

import { useState } from "react";

type Props = {
  deviceId: string;
  devicePublicKey: string;
  callback: string;
  state: string;
  accordVersion: string;
  accordHash: string;
};

async function requireJson(response: Response): Promise<unknown> {
  const value = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error((value as { error?: string }).error ?? "FTEP connection failed.");
  return value;
}

function base64Url(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

export function ConnectionClient(props: Props) {
  const [accepted, setAccepted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const complete = async () => {
    setBusy(true); setError(undefined);
    try {
      await requireJson(await fetch("/api/devices", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ deviceId: props.deviceId, publicKey: props.devicePublicKey, label: "SRE desktop" }) }));
      await requireJson(await fetch("/api/ratifications", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ deviceId: props.deviceId, version: props.accordVersion, documentHash: props.accordHash }) }));
      const lease = await requireJson(await fetch("/api/entitlements/lease", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ deviceId: props.deviceId }) }));
      const callback = new URL(props.callback);
      callback.searchParams.set("state", props.state);
      callback.searchParams.set("lease", base64Url(JSON.stringify(lease)));
      window.location.assign(callback);
    } catch (value) {
      setError(value instanceof Error ? value.message : String(value)); setBusy(false);
    }
  };
  return <article className="panel"><h3>Connect this SRE installation</h3><p className="muted">Register the random device public key, ratify Accord {props.accordVersion}, and return a short-lived device-bound lease through SRE&apos;s loopback callback.</p><label className="consent"><input type="checkbox" checked={accepted} onChange={(event) => setAccepted(event.target.checked)} /> I accept the Great Zelda–Satisfactory Accords. Real life overrides scheduling, and enforcement is non-destructive.</label>{error && <p className="error-text">{error}</p>}<div className="actions"><button className="button" disabled={!accepted || busy} onClick={() => void complete()}>{busy ? "Connecting…" : "Ratify and connect SRE"}</button></div></article>;
}
