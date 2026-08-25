import { auth } from "@/auth";
import { withAccountClient, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const metrics = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const result = await client.query(
        `SELECT
          (SELECT COUNT(*)::int FROM users) AS "accountCount",
          (SELECT COUNT(*)::int FROM devices WHERE revoked_at IS NULL) AS "activeDeviceCount",
          (SELECT COUNT(*)::int FROM entitlements WHERE state = 'ACTIVE') AS "activeEntitlementCount",
          (SELECT COUNT(*)::int FROM entitlements WHERE state = 'SUSPENDED') AS "suspendedEntitlementCount",
          (SELECT COUNT(*)::int FROM game_sessions WHERE state = 'ENDED') AS "completedSessionCount",
          (SELECT COUNT(*)::int FROM achievement_unlocks) AS "unlockCount",
          (SELECT COUNT(*)::int FROM audit_events WHERE created_at > now() - interval '24 hours') AS "auditEventsLast24Hours",
          (SELECT COUNT(*)::int FROM compliance_events WHERE state = 'MATERIAL_BREACH') AS "materialBreachCount"`,
      );
      return result.rows[0];
    });
    return jsonOk(request, { metrics });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP admin overview failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load the operations overview.");
  }
}
