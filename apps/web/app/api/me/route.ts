import { auth } from "@/auth";
import { withAccountClient } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const snapshot = await withAccountClient(session.user, async (client, account) => {
      const [entitlements, devices, compliance, sessions, unlocks] = await Promise.all([
        client.query(`SELECT entitlement_key AS "key", state, granted_at AS "grantedAt" FROM entitlements WHERE user_id = $1 ORDER BY entitlement_key`, [account.id]),
        client.query(`SELECT id AS "deviceId", label, registered_at AS "registeredAt", last_seen_at AS "lastSeenAt", revoked_at AS "revokedAt" FROM devices WHERE user_id = $1 ORDER BY registered_at DESC`, [account.id]),
        client.query(`SELECT state, details, recorded_at AS "recordedAt" FROM compliance_events WHERE user_id = $1 ORDER BY recorded_at DESC LIMIT 1`, [account.id]),
        client.query(`SELECT COUNT(*)::int AS count, COALESCE(SUM(duration_ms), 0)::bigint AS "durationMs" FROM game_sessions WHERE user_id = $1 AND state = 'ENDED'`, [account.id]),
        client.query(`SELECT COUNT(*)::int AS count FROM achievement_unlocks WHERE user_id = $1`, [account.id]),
      ]);
      return {
        account: { id: account.id, email: account.email, displayName: account.displayName, role: account.role },
        entitlements: entitlements.rows,
        devices: devices.rows,
        compliance: compliance.rows[0] ?? { state: "PENDING_ACCEPTANCE", details: {}, recordedAt: null },
        sessions: sessions.rows[0],
        achievements: unlocks.rows[0],
      };
    });
    return jsonOk(request, snapshot, { headers: { "Cache-Control": "private, no-store" } });
  } catch (error) {
    console.error("FTEP account snapshot failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load the account dashboard.");
  }
}
