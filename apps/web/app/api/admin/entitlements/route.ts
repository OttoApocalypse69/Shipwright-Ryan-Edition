import { auth } from "@/auth";
import { ingestAchievementEvent } from "@/lib/achievement-service";
import { withAccountClient, withAccountTransaction, recordAudit, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const actionSchema = z.object({
  action: z.enum(["suspend", "restore"]),
  userEmail: z.string().email(),
  entitlementKey: z.string().trim().min(1).max(120).default("ftep.library.nintendo"),
  reason: z.string().trim().min(3).max(1000),
});

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const url = new URL(request.url);
      const limit = Math.min(Math.max(Number.parseInt(url.searchParams.get("limit") ?? "100", 10) || 100, 1), 250);
      const rows = await client.query(
        `SELECT u.email, e.entitlement_key AS "entitlementKey", e.state, e.granted_at AS "grantedAt",
                ce.state AS "complianceState", ce.recorded_at AS "complianceRecordedAt"
           FROM entitlements e JOIN users u ON u.id = e.user_id
           LEFT JOIN LATERAL (SELECT state, recorded_at FROM compliance_events WHERE user_id = u.id ORDER BY recorded_at DESC LIMIT 1) ce ON true
          ORDER BY e.granted_at DESC LIMIT $1`,
        [limit],
      );
      return rows.rows;
    });
    return jsonOk(request, { entitlements: result });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP entitlement administration lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load entitlement administration.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof actionSchema>;
  try {
    input = actionSchema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0005", "Invalid entitlement operation.");
  }
  try {
    const result = await withAccountTransaction(session.user, async (client, admin) => {
      requireAdminAccount(admin);
      const target = await client.query<{ id: string; email: string }>("SELECT id, email FROM users WHERE email = $1 FOR UPDATE", [input.userEmail.trim().toLowerCase()]);
      if (target.rowCount !== 1) throw new Error("FTEP_ACCOUNT_NOT_FOUND");
      const entitlement = await client.query<{ id: string; state: string }>("SELECT id, state FROM entitlements WHERE user_id = $1 AND entitlement_key = $2 FOR UPDATE", [target.rows[0].id, input.entitlementKey]);
      if (entitlement.rowCount !== 1) throw new Error("FTEP_ENTITLEMENT_NOT_FOUND");
      if (input.action === "suspend") {
        await client.query("UPDATE entitlements SET state = 'SUSPENDED' WHERE id = $1", [entitlement.rows[0].id]);
        await client.query(
          `INSERT INTO revocations(lease_id, entitlement_id, reason, revoked_by)
           SELECT id, $1, $2, $3 FROM entitlement_leases WHERE entitlement_id = $1 AND revoked_at IS NULL`,
          [entitlement.rows[0].id, input.reason, admin.id],
        );
        await client.query("UPDATE entitlement_leases SET revoked_at = now() WHERE entitlement_id = $1 AND revoked_at IS NULL", [entitlement.rows[0].id]);
        await client.query("INSERT INTO compliance_events(user_id, state, details, recorded_by) VALUES ($1, 'MATERIAL_BREACH', $2::jsonb, $3)", [target.rows[0].id, JSON.stringify({ reason: input.reason, entitlementKey: input.entitlementKey }), admin.id]);
        await recordAudit(client, admin.id, "entitlement.suspend", "entitlement", entitlement.rows[0].id, { userEmail: target.rows[0].email, reason: input.reason });
      } else {
        await client.query("UPDATE entitlements SET state = 'ACTIVE', granted_at = now() WHERE id = $1", [entitlement.rows[0].id]);
        await client.query("INSERT INTO compliance_events(user_id, state, details, recorded_by) VALUES ($1, 'RESTORED', $2::jsonb, $3)", [target.rows[0].id, JSON.stringify({ reason: input.reason, entitlementKey: input.entitlementKey }), admin.id]);
        await ingestAchievementEvent(client, { ...admin, id: target.rows[0].id, email: target.rows[0].email }, {
          eventId: `compliance:restore:${entitlement.rows[0].id}:${Date.now()}`,
          eventType: "TREATY_STATE_CHANGED",
          occurredAt: new Date(),
          payload: { from: "MATERIAL_BREACH", to: "RESTORED", reason: input.reason },
          source: "ADMIN",
          schemaVersion: 1,
        });
        await recordAudit(client, admin.id, "entitlement.restore", "entitlement", entitlement.rows[0].id, { userEmail: target.rows[0].email, reason: input.reason });
      }
      return { email: target.rows[0].email, entitlementKey: input.entitlementKey, state: input.action === "suspend" ? "SUSPENDED" : "ACTIVE" };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    if (error instanceof Error && error.message === "FTEP_ACCOUNT_NOT_FOUND") return jsonError(request, 404, "FICSIT-0001", "Target account was not found.");
    if (error instanceof Error && error.message === "FTEP_ENTITLEMENT_NOT_FOUND") return jsonError(request, 404, "FICSIT-0005", "Target entitlement was not found.");
    console.error("FTEP entitlement administration failed", error);
    return jsonError(request, 500, "FICSIT-0005", "Entitlement operation failed.");
  }
}
