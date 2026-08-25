import { auth } from "@/auth";
import { ingestAchievementEvent } from "@/lib/achievement-service";
import { withAccountClient, withAccountTransaction, recordAudit, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const schema = z.object({
  userEmail: z.string().email(),
  state: z.enum(["WARNING", "MATERIAL_BREACH", "REMEDIATION", "RESTORED", "COMPLIANT"]),
  reason: z.string().trim().min(3).max(1000),
  incidentCode: z.string().trim().regex(/^FICSIT-[0-9]{4}$/).optional(),
});

const allowed: Record<string, string[]> = {
  PENDING_ACCEPTANCE: ["WARNING", "MATERIAL_BREACH", "COMPLIANT"],
  COMPLIANT: ["WARNING", "MATERIAL_BREACH"],
  WARNING: ["COMPLIANT", "MATERIAL_BREACH", "REMEDIATION"],
  MATERIAL_BREACH: ["REMEDIATION", "RESTORED"],
  REMEDIATION: ["RESTORED", "COMPLIANT"],
  RESTORED: ["COMPLIANT"],
};

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const url = new URL(request.url);
      const email = url.searchParams.get("userEmail")?.trim().toLowerCase();
      const rows = await client.query(
        `SELECT u.email, ce.state, ce.details, ce.recorded_at AS "recordedAt", actor.email AS "recordedBy"
           FROM compliance_events ce JOIN users u ON u.id = ce.user_id
           LEFT JOIN users actor ON actor.id = ce.recorded_by
          WHERE ($1::text IS NULL OR u.email = $1)
          ORDER BY ce.recorded_at DESC LIMIT 250`,
        [email ?? null],
      );
      return rows.rows;
    });
    return jsonOk(request, { compliance: result });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP compliance lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load compliance records.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try {
    input = schema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0001", "Invalid compliance transition.");
  }
  try {
    const result = await withAccountTransaction(session.user, async (client, admin) => {
      requireAdminAccount(admin);
      const target = await client.query<{ id: string; email: string }>("SELECT id, email FROM users WHERE email = $1 FOR UPDATE", [input.userEmail.trim().toLowerCase()]);
      if (target.rowCount !== 1) throw new Error("FTEP_ACCOUNT_NOT_FOUND");
      const previous = await client.query<{ state: string }>("SELECT state FROM compliance_events WHERE user_id = $1 ORDER BY recorded_at DESC LIMIT 1", [target.rows[0].id]);
      const priorState = previous.rows[0]?.state ?? "PENDING_ACCEPTANCE";
      if (!allowed[priorState]?.includes(input.state)) throw new Error("FTEP_INVALID_COMPLIANCE_TRANSITION");
      await client.query("INSERT INTO compliance_events(user_id, state, details, recorded_by) VALUES ($1, $2, $3::jsonb, $4)", [target.rows[0].id, input.state, JSON.stringify({ reason: input.reason, incidentCode: input.incidentCode ?? null, from: priorState }), admin.id]);
      if (input.state === "MATERIAL_BREACH") {
        const entitlements = await client.query<{ id: string }>("SELECT id FROM entitlements WHERE user_id = $1 AND state = 'ACTIVE' FOR UPDATE", [target.rows[0].id]);
        for (const entitlement of entitlements.rows) {
          await client.query("INSERT INTO revocations(entitlement_id, reason, revoked_by) VALUES ($1, $2, $3)", [entitlement.id, input.reason, admin.id]);
          await client.query("UPDATE entitlements SET state = 'SUSPENDED' WHERE id = $1", [entitlement.id]);
          await client.query("UPDATE entitlement_leases SET revoked_at = now() WHERE entitlement_id = $1 AND revoked_at IS NULL", [entitlement.id]);
        }
      }
      if (input.state === "RESTORED" || input.state === "COMPLIANT") {
        await client.query("UPDATE entitlements SET state = 'ACTIVE', granted_at = now() WHERE user_id = $1 AND state IN ('SUSPENDED', 'EXPIRED')", [target.rows[0].id]);
        await ingestAchievementEvent(client, { ...admin, id: target.rows[0].id, email: target.rows[0].email }, {
          eventId: `compliance:${target.rows[0].id}:${input.state}:${Date.now()}`,
          eventType: "TREATY_STATE_CHANGED",
          occurredAt: new Date(),
          payload: { from: priorState, to: input.state, reason: input.reason },
          source: "ADMIN",
          schemaVersion: 1,
        });
      }
      if (input.state === "MATERIAL_BREACH" && input.incidentCode) {
        await client.query("INSERT INTO incidents(user_id, code, severity, summary) VALUES ($1, $2, 'MATERIAL', $3)", [target.rows[0].id, input.incidentCode, input.reason]);
      }
      await recordAudit(client, admin.id, "compliance.transition", "user", target.rows[0].id, { from: priorState, to: input.state, reason: input.reason, incidentCode: input.incidentCode ?? null });
      return { email: target.rows[0].email, from: priorState, to: input.state };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    if (error instanceof Error && error.message === "FTEP_ACCOUNT_NOT_FOUND") return jsonError(request, 404, "FICSIT-0001", "Target account was not found.");
    if (error instanceof Error && error.message === "FTEP_INVALID_COMPLIANCE_TRANSITION") return jsonError(request, 409, "FICSIT-0005", "That compliance transition is not valid from the current state.");
    console.error("FTEP compliance transition failed", error);
    return jsonError(request, 500, "FICSIT-0005", "Compliance transition failed.");
  }
}
