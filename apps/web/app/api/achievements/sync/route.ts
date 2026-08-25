import { achievementDefinitions } from "@/lib/achievement-service";
import { withAccountTransaction, recordAudit } from "@/lib/control-plane";
import { apiIdentity, readRequestBody } from "@/lib/device-auth";
import { jsonError, jsonOk } from "@/lib/http";
import { z } from "zod";

const knownAchievements = new Set<string>(achievementDefinitions.map((definition) => definition.id));
const schema = z.object({
  deviceId: z.string().uuid(),
  unlocks: z.array(z.object({
    achievementId: z.string().trim().min(1).max(40),
    unlockId: z.string().trim().min(8).max(180).regex(/^[A-Za-z0-9._:-]+$/),
    unlockedAtUnixMs: z.number().int().positive(),
  })).max(100),
});

export async function POST(request: Request) {
  try {
    const parsedBody = await readRequestBody(request);
    const identity = await apiIdentity(request, parsedBody.body);
    if (!identity) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
    const input = schema.parse(parsedBody.json);
    if (identity.via === "DEVICE_SIGNATURE" && identity.deviceId !== input.deviceId) return jsonError(request, 403, "FICSIT-0004", "The achievement device does not match the authenticated device.");
    const result = await withAccountTransaction(identity, async (client, account) => {
      const device = await client.query("SELECT 1 FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL", [input.deviceId, account.id]);
      if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_REGISTERED");
      const synchronized: string[] = [];
      for (const unlock of input.unlocks) {
        if (!knownAchievements.has(unlock.achievementId)) continue;
        const result = await client.query(
          `INSERT INTO achievement_unlocks(user_id, achievement_id, unlocked_at, client_event_id)
           VALUES ($1, $2, to_timestamp($3 / 1000.0), $4)
           ON CONFLICT DO NOTHING`,
          [account.id, unlock.achievementId, unlock.unlockedAtUnixMs, unlock.unlockId],
        );
        if (result.rowCount === 1) synchronized.push(unlock.achievementId);
      }
      await recordAudit(client, account.id, "achievement.sync", "device", input.deviceId, {
        submitted: input.unlocks.length,
        synchronized: synchronized.length,
      });
      return { accepted: true as const, synchronized };
    });
    return jsonOk(request, result, { status: 202 });
  } catch (error) {
    if (error instanceof z.ZodError) return jsonError(request, 400, "FICSIT-0001", "Invalid achievement synchronization payload.");
    if (error instanceof SyntaxError) return jsonError(request, 400, "FICSIT-0001", "Invalid JSON request.");
    if (error instanceof Error && error.message === "FTEP_REQUEST_TOO_LARGE") return jsonError(request, 413, "FICSIT-0001", "Request is too large.");
    if (error instanceof Error && ["FTEP_DEVICE_REQUEST_EXPIRED", "FTEP_DEVICE_REQUEST_REPLAYED", "FTEP_DEVICE_SIGNATURE_INVALID", "FTEP_DEVICE_NOT_REGISTERED"].includes(error.message)) return jsonError(request, 401, "FICSIT-0003", "Device authentication failed.");
    console.error("FTEP achievement synchronization failed", error);
    return jsonError(request, 500, "FICSIT-0001", "Achievement synchronization failed.");
  }
}
