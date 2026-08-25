import { auth } from "@/auth";
import { withAccountClient, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const users = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const rows = await client.query(
        `SELECT u.id, u.email, u.display_name AS "displayName", u.role, u.created_at AS "createdAt", u.last_authenticated_at AS "lastAuthenticatedAt",
                (SELECT COUNT(*)::int FROM devices d WHERE d.user_id = u.id AND d.revoked_at IS NULL) AS "activeDevices",
                (SELECT COUNT(*)::int FROM achievement_unlocks au WHERE au.user_id = u.id) AS "unlockedAchievements",
                (SELECT COUNT(*)::int FROM game_sessions gs WHERE gs.user_id = u.id AND gs.state = 'ENDED') AS "completedSessions",
                (SELECT state FROM entitlements e WHERE e.user_id = u.id AND e.entitlement_key = 'ftep.library.nintendo') AS "entitlementState"
           FROM users u ORDER BY u.created_at DESC LIMIT 250`,
      );
      return rows.rows;
    });
    return jsonOk(request, { users });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP user administration lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load user administration.");
  }
}
