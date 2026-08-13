import { auth } from "@/auth";
import { requireAdmin } from "@/lib/authorization";
import { database } from "@/lib/db";
import { z } from "zod";

const requestSchema = z.object({ gameVariantId: z.string().uuid(), runtimeId: z.string().regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/), versionRange: z.string().min(1).max(100), status: z.enum(["UNSUPPORTED","INVESTIGATING","EXPERIMENTAL","SUPPORTED","DEGRADED","BROKEN","DEPRECATED"]), notes: z.string().max(2000).optional() });

export async function POST(request: Request) {
  const session = await auth();
  try { requireAdmin(session?.user.role ?? "user"); } catch { return Response.json({ error: "Administrator role required." }, { status: 403 }); }
  const parsed = requestSchema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) return Response.json({ error: "Invalid compatibility update.", issues: parsed.error.issues }, { status: 400 });
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    await client.query("INSERT INTO runtime_compatibility(game_variant_id,runtime_id,version_range,status,notes,updated_by) SELECT $1,$2,$3,$4,$5,id FROM users WHERE email=$6 ON CONFLICT(game_variant_id,runtime_id,version_range) DO UPDATE SET status=EXCLUDED.status,notes=EXCLUDED.notes,updated_by=EXCLUDED.updated_by,updated_at=now()", [parsed.data.gameVariantId, parsed.data.runtimeId, parsed.data.versionRange, parsed.data.status, parsed.data.notes ?? null, session?.user.email]);
    await client.query("INSERT INTO audit_events(actor_user_id,action,target_type,target_id,metadata) SELECT id,'compatibility.update','runtime_compatibility',$1,$2::jsonb FROM users WHERE email=$3", [`${parsed.data.gameVariantId}:${parsed.data.runtimeId}`, JSON.stringify({ versionRange: parsed.data.versionRange, status: parsed.data.status }), session?.user.email]);
    await client.query("COMMIT");
    return Response.json({ ok: true });
  } catch (error) { await client.query("ROLLBACK"); return Response.json({ error: "Compatibility update failed.", requestId: crypto.randomUUID() }, { status: 500 }); }
  finally { client.release(); }
}
