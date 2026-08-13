import { auth } from "@/auth";
import { database } from "@/lib/db";
import { signLease } from "@/lib/signing";
import { z } from "zod";

const schema = z.object({ deviceId: z.string().uuid() });

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return Response.json({ error: "Authentication required." }, { status: 401 });
  const parsed = schema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) return Response.json({ error: "Invalid lease request." }, { status: 400 });
  const client = await database().connect();
  try {
    const result = await client.query<{ user_id: string; entitlement_id: string; entitlement_key: string }>(
      "SELECT d.user_id,e.id AS entitlement_id,e.entitlement_key FROM devices d JOIN users u ON u.id=d.user_id JOIN entitlements e ON e.user_id=u.id AND e.state='ACTIVE' WHERE d.id=$1 AND d.revoked_at IS NULL AND u.email=$2 ORDER BY e.entitlement_key",
      [parsed.data.deviceId, session.user.email],
    );
    if (result.rows.length === 0) return Response.json({ error: "No active entitlement is available for this registered device." }, { status: 403 });
    const now = Math.floor(Date.now() / 1000); const leaseId = crypto.randomUUID();
    const payload = { leaseId, subjectId: result.rows[0].user_id, deviceId: parsed.data.deviceId, entitlements: result.rows.map((row) => row.entitlement_key), issuedAtUnixSecs: now, notBeforeUnixSecs: now - 30, expiresAtUnixSecs: now + 24 * 60 * 60 };
    const signed = signLease(payload);
    await client.query("INSERT INTO entitlement_leases(id,entitlement_id,device_id,key_id,issued_at,not_before,expires_at,signed_payload,signature) VALUES($1,$2,$3,$4,to_timestamp($5),to_timestamp($6),to_timestamp($7),$8,$9)", [leaseId, result.rows[0].entitlement_id, parsed.data.deviceId, signed.keyId, payload.issuedAtUnixSecs, payload.notBeforeUnixSecs, payload.expiresAtUnixSecs, signed.payload, signed.signature]);
    return Response.json(signed, { headers: { "Cache-Control": "no-store" } });
  } catch { return Response.json({ error: "Lease issuance failed.", requestId: crypto.randomUUID() }, { status: 500 }); }
  finally { client.release(); }
}
