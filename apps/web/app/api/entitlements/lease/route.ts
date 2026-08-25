import { auth } from "@/auth";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { withAccountTransaction, recordAudit } from "@/lib/control-plane";
import { isLocalDatabaseUnavailable } from "@/lib/local-credentials";
import { issueLocalLease } from "@/lib/local-ftep-state";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { signLease } from "@/lib/signing";
import { z } from "zod";

const schema = z.object({ deviceId: z.string().uuid() });

function leaseTtlSeconds(): number {
  const configured = Number.parseInt(process.env.FTEP_LEASE_TTL_SECS ?? "86400", 10);
  return Number.isFinite(configured) && configured >= 900 && configured <= 2_592_000 ? configured : 86_400;
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try {
    input = schema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0005", "Invalid lease request.");
  }

  try {
    const signed = await withAccountTransaction(session.user, async (client, account) => {
      const result = await client.query<{ user_id: string; entitlement_id: string; entitlement_key: string }>(
        `SELECT d.user_id, e.id AS entitlement_id, e.entitlement_key
           FROM devices d
           JOIN entitlements e ON e.user_id = d.user_id AND e.state = 'ACTIVE'
          WHERE d.id = $1 AND d.user_id = $2 AND d.revoked_at IS NULL
            AND NOT EXISTS (SELECT 1 FROM revocations r WHERE r.entitlement_id = e.id AND r.revoked_at >= e.granted_at)
          ORDER BY e.entitlement_key`,
        [input.deviceId, account.id],
      );
      if (result.rows.length === 0) throw new Error("FTEP_NO_ACTIVE_ENTITLEMENT");
      const now = Math.floor(Date.now() / 1000);
      const payload = {
        leaseId: crypto.randomUUID(),
        subjectId: result.rows[0].user_id,
        deviceId: input.deviceId,
        entitlements: result.rows.map((row) => row.entitlement_key),
        issuedAtUnixSecs: now,
        notBeforeUnixSecs: now - 30,
        expiresAtUnixSecs: now + leaseTtlSeconds(),
      };
      const signedLease = signLease(payload);
      await client.query(
        `INSERT INTO entitlement_leases(id, entitlement_id, device_id, key_id, issued_at, not_before, expires_at, signed_payload, signature, revoked_at)
         VALUES ($1, $2, $3, $4, to_timestamp($5), to_timestamp($6), to_timestamp($7), $8, $9, NULL)`,
        [payload.leaseId, result.rows[0].entitlement_id, input.deviceId, signedLease.keyId, payload.issuedAtUnixSecs, payload.notBeforeUnixSecs, payload.expiresAtUnixSecs, signedLease.payload, signedLease.signature],
      );
      await client.query("UPDATE devices SET last_seen_at = now() WHERE id = $1", [input.deviceId]);
      await recordAudit(client, account.id, "entitlement.lease.issue", "device", input.deviceId, {
        leaseId: payload.leaseId,
        entitlementCount: payload.entitlements.length,
        expiresAtUnixSecs: payload.expiresAtUnixSecs,
      });
      return signedLease;
    });
    return jsonOk(request, signed, { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_NO_ACTIVE_ENTITLEMENT") return jsonError(request, 403, "FICSIT-0005", "No active entitlement is available for this registered device.");
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      try {
        const signed = await issueLocalLease({ email: session.user.email, deviceId: input.deviceId });
        return jsonOk(request, signed, { headers: { "Cache-Control": "no-store", "X-FTEP-Storage": "local-development" } });
      } catch (localError) {
        console.error("FTEP local lease issuance failed", localError);
        return jsonError(request, 500, "FICSIT-0005", "Local lease issuance failed.");
      }
    }
    console.error("FTEP lease issuance failed", error);
    return jsonError(request, 500, "FICSIT-0005", "Lease issuance failed.");
  }
}
