import { auth } from "@/auth";
import { ingestAchievementEvent } from "@/lib/achievement-service";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { withAccountClient, withAccountTransaction, recordAudit, isUniqueViolation } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { isLocalDatabaseUnavailable } from "@/lib/local-credentials";
import { readLocalFtepState, registerLocalDevice, revokeLocalDevice } from "@/lib/local-ftep-state";
import { z } from "zod";

const registrationSchema = z.object({
  deviceId: z.string().uuid(),
  publicKey: z.string().regex(/^[A-Za-z0-9_-]{43}$/),
  label: z.string().trim().min(1).max(80).optional(),
});
const deviceActionSchema = z.object({ deviceId: z.string().uuid() });

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const devices = await withAccountClient(session.user, async (client, account) => {
      const result = await client.query(
        `SELECT d.id AS "deviceId", d.label, d.public_key AS "publicKey", d.registered_at AS "registeredAt",
                d.last_seen_at AS "lastSeenAt", d.revoked_at AS "revokedAt", d.revoked_reason AS "revokedReason",
                COUNT(l.id)::int AS "leaseCount"
           FROM devices d
           LEFT JOIN entitlement_leases l ON l.device_id = d.id
          WHERE d.user_id = $1
          GROUP BY d.id
          ORDER BY d.registered_at DESC`,
        [account.id],
      );
      return result.rows;
    });
    return jsonOk(request, { devices });
  } catch (error) {
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      const state = await readLocalFtepState();
      const email = session.user.email.trim().toLowerCase();
      return jsonOk(request, {
        devices: state.devices.filter((device) => device.email === email).map((device) => ({
          deviceId: device.deviceId,
          label: device.label,
          publicKey: device.publicKey,
          registeredAt: new Date(device.registeredAtUnixMs).toISOString(),
          lastSeenAt: null,
          revokedAt: device.revokedAtUnixMs ? new Date(device.revokedAtUnixMs).toISOString() : null,
          revokedReason: null,
          leaseCount: state.leases.filter((lease) => lease.deviceId === device.deviceId).length,
        })),
      }, { headers: { "X-FTEP-Storage": "local-development" } });
    }
    console.error("FTEP device list failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load devices.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof registrationSchema>;
  try {
    input = registrationSchema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0004", "Invalid device registration.");
  }

  try {
    const result = await withAccountTransaction(session.user, async (client, account) => {
      const existing = await client.query<{ user_id: string; public_key: string; revoked_at: string | null }>(
        "SELECT user_id, public_key, revoked_at FROM devices WHERE id = $1 FOR UPDATE",
        [input.deviceId],
      );
      if (existing.rows[0] && (existing.rows[0].user_id !== account.id || existing.rows[0].public_key !== input.publicKey)) {
        throw new Error("FTEP_DEVICE_OWNERSHIP_CONFLICT");
      }
      await client.query(
        `INSERT INTO devices(id, user_id, public_key, label, registered_at, last_seen_at, revoked_at, revoked_reason)
         VALUES ($1, $2, $3, $4, now(), now(), NULL, NULL)
         ON CONFLICT(id) DO UPDATE SET
           label = EXCLUDED.label,
           last_seen_at = now(),
           revoked_at = NULL,
           revoked_reason = NULL`,
        [input.deviceId, account.id, input.publicKey, input.label ?? null],
      );
      await recordAudit(client, account.id, "device.register", "device", input.deviceId, { label: input.label ?? null });
      await ingestAchievementEvent(client, account, {
        eventId: `device:${input.deviceId}`,
        eventType: "DEVICE_REGISTERED",
        deviceId: input.deviceId,
        occurredAt: new Date(),
        payload: { deviceId: input.deviceId },
        source: "FTEP",
        schemaVersion: 1,
      });
      return { deviceId: input.deviceId, registered: true as const };
    });
    return jsonOk(request, result, { status: 201 });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_DEVICE_OWNERSHIP_CONFLICT") {
      return jsonError(request, 409, "FICSIT-0004", "This device identity is already bound to another account or key.", { reason: "DEVICE_OWNERSHIP_CONFLICT" });
    }
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      try {
        const result = await registerLocalDevice({ email: session.user.email, deviceId: input.deviceId, publicKey: input.publicKey, label: input.label });
        return jsonOk(request, result, { status: 201, headers: { "X-FTEP-Storage": "local-development" } });
      } catch (localError) {
        console.error("FTEP local device registration failed", localError);
        return jsonError(request, 500, "FICSIT-0004", "Local device registration failed.");
      }
    }
    if (isUniqueViolation(error)) return jsonError(request, 409, "FICSIT-0004", "This device public key is already registered.", { reason: "DEVICE_KEY_ALREADY_REGISTERED" });
    console.error("FTEP device registration failed", error);
    return jsonError(request, 500, "FICSIT-0004", "Device registration failed.");
  }
}

export async function DELETE(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof deviceActionSchema>;
  try {
    input = deviceActionSchema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0004", "Invalid device action.");
  }
  try {
    const result = await withAccountTransaction(session.user, async (client, account) => {
      const device = await client.query<{ id: string }>("SELECT id FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL FOR UPDATE", [input.deviceId, account.id]);
      if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_FOUND");
      await client.query("UPDATE devices SET revoked_at = now(), revoked_reason = 'USER_REQUESTED', last_seen_at = now() WHERE id = $1", [input.deviceId]);
      await client.query(
        `INSERT INTO revocations(lease_id, reason, revoked_by)
         SELECT id, 'Device revoked by account owner', $2 FROM entitlement_leases WHERE device_id = $1 AND revoked_at IS NULL`,
        [input.deviceId, account.id],
      );
      await client.query("UPDATE entitlement_leases SET revoked_at = now() WHERE device_id = $1 AND revoked_at IS NULL", [input.deviceId]);
      await recordAudit(client, account.id, "device.revoke", "device", input.deviceId, { reason: "USER_REQUESTED" });
      return { deviceId: input.deviceId, revoked: true as const };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_DEVICE_NOT_FOUND") return jsonError(request, 404, "FICSIT-0004", "That device is not registered or is already revoked.");
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      try {
        const result = await revokeLocalDevice({ email: session.user.email, deviceId: input.deviceId });
        return jsonOk(request, result, { headers: { "X-FTEP-Storage": "local-development" } });
      } catch (localError) {
        if (localError instanceof Error && localError.message === "FTEP_DEVICE_NOT_FOUND") return jsonError(request, 404, "FICSIT-0004", "That device is not registered or is already revoked.");
        console.error("FTEP local device revocation failed", localError);
        return jsonError(request, 500, "FICSIT-0004", "Local device revocation failed.");
      }
    }
    console.error("FTEP device revocation failed", error);
    return jsonError(request, 500, "FICSIT-0004", "Device revocation failed.");
  }
}
