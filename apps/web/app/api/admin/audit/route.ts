import { auth } from "@/auth";
import { withAccountClient, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const events = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const url = new URL(request.url);
      const limit = Math.min(Math.max(Number.parseInt(url.searchParams.get("limit") ?? "100", 10) || 100, 1), 250);
      const action = url.searchParams.get("action");
      const rows = await client.query(
        `SELECT ae.id, ae.action, ae.target_type AS "targetType", ae.target_id AS "targetId", ae.request_id AS "requestId", ae.metadata, ae.created_at AS "createdAt", u.email AS actor
           FROM audit_events ae LEFT JOIN users u ON u.id = ae.actor_user_id
          WHERE ($1::text IS NULL OR ae.action = $1)
          ORDER BY ae.created_at DESC LIMIT $2`,
        [action || null, limit],
      );
      return rows.rows;
    });
    return jsonOk(request, { events });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP audit lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load the audit history.");
  }
}
