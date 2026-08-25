import { auth } from "@/auth";
import { withAccountClient, withAccountTransaction, recordAudit, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const schema = z.object({ deviceId: z.string().uuid(), reason: z.string().trim().min(3).max(1000) });

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const devices = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const rows = await client.query(
        `SELECT d.id AS "deviceId", u.email, d.label, d.public_key AS "publicKey", d.registered_at AS "registeredAt", d.last_seen_at AS "lastSeenAt", d.revoked_at AS "revokedAt", d.revoked_reason AS "revokedReason"
           FROM devices d JOIN users u ON u.id = d.user_id ORDER BY d.registered_at DESC LIMIT 250`,
      );
      return rows.rows;
    });
    return jsonOk(request, { devices });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP admin device lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load device administration.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try { input = schema.parse(await requestJson(request)); } catch { return jsonError(request, 400, "FICSIT-0004", "Invalid device revocation."); }
  try {
    const result = await withAccountTransaction(session.user, async (client, admin) => {
      requireAdminAccount(admin);
      const device = await client.query<{ id: string; user_id: string }>("SELECT id, user_id FROM devices WHERE id = $1 FOR UPDATE", [input.deviceId]);
      if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_FOUND");
      await client.query("UPDATE devices SET revoked_at = now(), revoked_reason = $2 WHERE id = $1", [input.deviceId, input.reason]);
      await client.query("INSERT INTO revocations(lease_id, reason, revoked_by) SELECT id, $2, $3 FROM entitlement_leases WHERE device_id = $1 AND revoked_at IS NULL", [input.deviceId, input.reason, admin.id]);
      await client.query("UPDATE entitlement_leases SET revoked_at = now() WHERE device_id = $1 AND revoked_at IS NULL", [input.deviceId]);
      await recordAudit(client, admin.id, "device.revoke.admin", "device", input.deviceId, { reason: input.reason, ownerUserId: device.rows[0].user_id });
      return { deviceId: input.deviceId, revoked: true as const };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    if (error instanceof Error && error.message === "FTEP_DEVICE_NOT_FOUND") return jsonError(request, 404, "FICSIT-0004", "Device was not found.");
    console.error("FTEP admin device revocation failed", error);
    return jsonError(request, 500, "FICSIT-0004", "Device revocation failed.");
  }
}
