import { auth } from "@/auth";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { database } from "@/lib/db";
import { z } from "zod";

const schema = z.object({ deviceId: z.string().uuid(), publicKey: z.string().regex(/^[A-Za-z0-9_-]{43}$/), label: z.string().min(1).max(80).optional() });

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return Response.json({ error: "Authentication required." }, { status: 401 });
  const parsed = schema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) return Response.json({ error: "Invalid device registration." }, { status: 400 });
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const user = await client.query<{ id: string }>("INSERT INTO users(email,display_name) VALUES($1,$2) ON CONFLICT(email) DO UPDATE SET display_name=COALESCE(EXCLUDED.display_name,users.display_name),updated_at=now() RETURNING id", [session.user.email, session.user.name]);
    let device = await client.query("INSERT INTO devices(id,user_id,public_key,label) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO UPDATE SET label=EXCLUDED.label WHERE devices.user_id=EXCLUDED.user_id AND devices.public_key=EXCLUDED.public_key RETURNING id", [parsed.data.deviceId, user.rows[0].id, parsed.data.publicKey, parsed.data.label ?? null]);
    if (device.rowCount !== 1 && localCredentialsEnabled()) {
      // A development-only local account can be recreated while SRE retains its
      // durable device identity. Rebind that local identity instead of trapping
      // the user behind an old, unusable development account.
      device = await client.query("UPDATE devices SET user_id=$2,public_key=$3,label=$4,registered_at=now(),revoked_at=NULL WHERE id=$1 RETURNING id", [parsed.data.deviceId, user.rows[0].id, parsed.data.publicKey, parsed.data.label ?? null]);
    }
    if (device.rowCount !== 1) throw new Error("Device identity is already bound to another account or key");
    await client.query("INSERT INTO audit_events(actor_user_id,action,target_type,target_id) VALUES($1,'device.register','device',$2)", [user.rows[0].id, parsed.data.deviceId]);
    await client.query("COMMIT");
    return Response.json({ deviceId: parsed.data.deviceId, registered: true }, { status: 201 });
  } catch (error) {
    console.error("FTEP device registration database error", error);
    await client.query("ROLLBACK").catch(() => undefined);
    return Response.json({ error: "Device registration failed.", requestId: crypto.randomUUID() }, { status: 500 });
  }
  finally { client.release(); }
}
