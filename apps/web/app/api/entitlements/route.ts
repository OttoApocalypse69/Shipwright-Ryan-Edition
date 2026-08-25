import { auth } from "@/auth";
import { withAccountClient } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      const [entitlements, leases] = await Promise.all([
        client.query(`SELECT entitlement_key AS "key", state, granted_at AS "grantedAt" FROM entitlements WHERE user_id = $1 ORDER BY entitlement_key`, [account.id]),
        client.query(`SELECT el.id AS "leaseId", el.device_id AS "deviceId", el.key_id AS "keyId", el.issued_at AS "issuedAt", el.not_before AS "notBefore", el.expires_at AS "expiresAt", el.revoked_at AS "revokedAt", d.label AS "deviceLabel" FROM entitlement_leases el JOIN devices d ON d.id = el.device_id JOIN entitlements e ON e.id = el.entitlement_id WHERE e.user_id = $1 ORDER BY el.issued_at DESC LIMIT 100`, [account.id]),
      ]);
      return { entitlements: entitlements.rows, leases: leases.rows };
    });
    return jsonOk(request, result, { headers: { "Cache-Control": "private, no-store" } });
  } catch (error) {
    console.error("FTEP entitlement lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load entitlement state.");
  }
}
