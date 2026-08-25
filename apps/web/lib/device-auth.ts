import { createHash, createPublicKey, randomUUID, verify } from "node:crypto";
import { auth } from "@/auth";
import { database } from "@/lib/db";

const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");

export type ApiIdentity = {
  email: string;
  name?: string | null;
  deviceId?: string;
  via: "SESSION" | "DEVICE_SIGNATURE";
};

function requestWindowSeconds(): number {
  const configured = Number.parseInt(process.env.FTEP_DEVICE_REQUEST_WINDOW_SECS ?? "300", 10);
  return Number.isFinite(configured) && configured >= 30 && configured <= 900 ? configured : 300;
}

export async function apiIdentity(request: Request, body: string): Promise<ApiIdentity | null> {
  const session = await auth();
  if (session?.user.email) return { email: session.user.email, name: session.user.name, via: "SESSION" };
  const deviceId = request.headers.get("x-ftep-device-id")?.trim();
  const timestamp = request.headers.get("x-ftep-device-timestamp")?.trim();
  const nonce = request.headers.get("x-ftep-device-nonce")?.trim();
  const signatureValue = request.headers.get("x-ftep-device-signature")?.trim();
  if (!deviceId || !timestamp || !nonce || !signatureValue || !/^[0-9a-f-]{36}$/i.test(deviceId) || !/^[0-9a-f-]{36}$/i.test(nonce)) return null;
  if (!/^\d{1,12}$/.test(timestamp)) throw new Error("FTEP_DEVICE_REQUEST_EXPIRED");
  const timestampSeconds = Number.parseInt(timestamp, 10);
  if (!Number.isInteger(timestampSeconds) || Math.abs(Math.floor(Date.now() / 1000) - timestampSeconds) > requestWindowSeconds()) throw new Error("FTEP_DEVICE_REQUEST_EXPIRED");
  const bodyHash = createHash("sha256").update(body).digest("hex");
  const message = `${request.method}\n${new URL(request.url).pathname}\n${timestampSeconds}\n${nonce}\n${bodyHash}`;
  let signature: Buffer;
  try { signature = Buffer.from(signatureValue, "base64url"); } catch { throw new Error("FTEP_DEVICE_SIGNATURE_INVALID"); }
  if (signature.length !== 64) throw new Error("FTEP_DEVICE_SIGNATURE_INVALID");
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const device = await client.query<{ email: string; public_key: string; name: string | null }>(
      `SELECT u.email, d.public_key, u.display_name AS name
         FROM devices d JOIN users u ON u.id = d.user_id
        WHERE d.id = $1 AND d.revoked_at IS NULL`,
      [deviceId],
    );
    const record = device.rows[0];
    if (!record) throw new Error("FTEP_DEVICE_NOT_REGISTERED");
    const rawPublicKey = Buffer.from(record.public_key, "base64url");
    if (rawPublicKey.length !== 32) throw new Error("FTEP_DEVICE_SIGNATURE_INVALID");
    let publicKey: ReturnType<typeof createPublicKey>;
    try {
      publicKey = createPublicKey({ key: Buffer.concat([spkiPrefix, rawPublicKey]), format: "der", type: "spki" });
    } catch {
      throw new Error("FTEP_DEVICE_SIGNATURE_INVALID");
    }
    if (!verify(null, Buffer.from(message), publicKey, signature)) throw new Error("FTEP_DEVICE_SIGNATURE_INVALID");
    const nonceResult = await client.query("INSERT INTO device_request_nonces(device_id, nonce) VALUES ($1, $2) ON CONFLICT DO NOTHING", [deviceId, nonce]);
    if (nonceResult.rowCount !== 1) throw new Error("FTEP_DEVICE_REQUEST_REPLAYED");
    await client.query("DELETE FROM device_request_nonces WHERE seen_at < now() - interval '2 days'");
    await client.query("UPDATE devices SET last_seen_at = now() WHERE id = $1", [deviceId]);
    await client.query("COMMIT");
    return { email: record.email, name: record.name, deviceId, via: "DEVICE_SIGNATURE" };
  } catch (error) {
    await client.query("ROLLBACK").catch(() => undefined);
    throw error;
  } finally {
    client.release();
  }
}

export async function readRequestBody(request: Request): Promise<{ body: string; json: unknown }> {
  const body = await request.text();
  if (Buffer.byteLength(body, "utf8") > 1_048_576) throw new Error("FTEP_REQUEST_TOO_LARGE");
  return { body, json: body ? JSON.parse(body) : null };
}

export function requestNonce(): string { return randomUUID(); }
