import { auth } from "@/auth";
import { accord, accordDocumentHash } from "@/lib/accord";
import { database } from "@/lib/db";
import { z } from "zod";

const schema = z.object({
  deviceId: z.string().uuid(),
  version: z.literal(accord.version),
  documentHash: z.literal(accordDocumentHash),
});

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return Response.json({ error: "Authentication required." }, { status: 401 });
  const parsed = schema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) return Response.json({ error: "The Accord identity is invalid." }, { status: 400 });
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const user = await client.query<{ id: string }>("SELECT id FROM users WHERE email=$1", [session.user.email]);
    if (user.rowCount !== 1) throw new Error("Account is not registered");
    const device = await client.query("SELECT 1 FROM devices WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL", [parsed.data.deviceId, user.rows[0].id]);
    if (device.rowCount !== 1) throw new Error("Device is not registered to this account");
    await client.query("INSERT INTO treaties(id,title) VALUES($1,$2) ON CONFLICT(id) DO UPDATE SET title=EXCLUDED.title", [accord.treatyId, accord.title]);
    const treatyVersion = await client.query<{ id: string }>("INSERT INTO treaty_versions(treaty_id,semantic_version,document_hash,structured_document,published_at) VALUES($1,$2,$3,$4::jsonb,now()) ON CONFLICT(treaty_id,semantic_version) DO UPDATE SET document_hash=EXCLUDED.document_hash,structured_document=EXCLUDED.structured_document RETURNING id", [accord.treatyId, accord.version, accordDocumentHash, JSON.stringify(accord)]);
    await client.query("UPDATE treaties SET active_version_id=$1 WHERE id=$2", [treatyVersion.rows[0].id, accord.treatyId]);
    for (const article of accord.articles) {
      await client.query("INSERT INTO treaty_articles(treaty_version_id,article_number,title,body) VALUES($1,$2,$3,$4) ON CONFLICT(treaty_version_id,article_number) DO UPDATE SET title=EXCLUDED.title,body=EXCLUDED.body", [treatyVersion.rows[0].id, article.number, article.title, article.summary]);
    }
    await client.query("INSERT INTO ratifications(treaty_version_id,user_id,accepted_at,document_hash) VALUES($1,$2,now(),$3) ON CONFLICT(treaty_version_id,user_id) DO UPDATE SET accepted_at=EXCLUDED.accepted_at,document_hash=EXCLUDED.document_hash", [treatyVersion.rows[0].id, user.rows[0].id, accordDocumentHash]);
    await client.query("INSERT INTO entitlements(user_id,entitlement_key,state) VALUES($1,'ftep.library.nintendo','ACTIVE') ON CONFLICT(user_id,entitlement_key) DO UPDATE SET state='ACTIVE',granted_at=now()", [user.rows[0].id]);
    await client.query("INSERT INTO audit_events(actor_user_id,action,target_type,target_id,metadata) VALUES($1,'treaty.ratify','treaty',$2,jsonb_build_object('version',$3::text,'deviceId',$4::text))", [user.rows[0].id, accord.treatyId, accord.version, parsed.data.deviceId]);
    await client.query("COMMIT");
    return Response.json({ treatyId: accord.treatyId, version: accord.version, documentHash: accordDocumentHash, ratified: true });
  } catch (error) {
    console.error("FTEP Accord ratification database error", error);
    await client.query("ROLLBACK");
    return Response.json({ error: "Accord ratification failed.", requestId: crypto.randomUUID() }, { status: 500 });
  } finally {
    client.release();
  }
}
