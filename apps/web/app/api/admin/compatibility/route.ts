import { auth } from "@/auth";
import { withAccountClient, withAccountTransaction, recordAudit, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const requestSchema = z.object({
  gameVariantId: z.string().uuid(),
  runtimeId: z.string().regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/),
  versionRange: z.string().trim().min(1).max(100),
  status: z.enum(["UNSUPPORTED", "INVESTIGATING", "EXPERIMENTAL", "SUPPORTED", "DEGRADED", "BROKEN", "DEPRECATED"]),
  notes: z.string().trim().max(2000).optional(),
});

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const rows = await withAccountClient(session.user, async (client, account) => {
      requireAdminAccount(account);
      const result = await client.query(
        `SELECT rc.id, g.id AS "gameId", g.title, gv.id AS "gameVariantId", gv.variant_key AS "variantId", rc.runtime_id AS "runtimeId", rc.version_range AS "versionRange", rc.status, rc.notes, rc.updated_at AS "updatedAt", u.email AS "updatedBy"
           FROM runtime_compatibility rc JOIN game_variants gv ON gv.id = rc.game_variant_id JOIN games g ON g.id = gv.game_id LEFT JOIN users u ON u.id = rc.updated_by
          ORDER BY g.title, gv.variant_key, rc.runtime_id`,
      );
      return result.rows;
    });
    return jsonOk(request, { compatibility: rows });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    console.error("FTEP admin compatibility lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load compatibility administration.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof requestSchema>;
  try { input = requestSchema.parse(await requestJson(request)); } catch { return jsonError(request, 400, "FICSIT-0001", "Invalid compatibility update."); }
  try {
    const result = await withAccountTransaction(session.user, async (client, account) => {
      requireAdminAccount(account);
      const variant = await client.query("SELECT 1 FROM game_variants WHERE id = $1", [input.gameVariantId]);
      if (variant.rowCount !== 1) throw new Error("FTEP_VARIANT_NOT_FOUND");
      await client.query(
        `INSERT INTO runtime_compatibility(game_variant_id, runtime_id, version_range, status, notes, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT(game_variant_id, runtime_id, version_range) DO UPDATE SET status = EXCLUDED.status, notes = EXCLUDED.notes, updated_by = EXCLUDED.updated_by, updated_at = now()`,
        [input.gameVariantId, input.runtimeId, input.versionRange, input.status, input.notes ?? null, account.id],
      );
      await recordAudit(client, account.id, "compatibility.update", "runtime_compatibility", `${input.gameVariantId}:${input.runtimeId}`, input);
      return { ok: true as const };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required.");
    if (error instanceof Error && error.message === "FTEP_VARIANT_NOT_FOUND") return jsonError(request, 404, "FICSIT-0007", "Game variant was not found.");
    console.error("FTEP compatibility update failed", error);
    return jsonError(request, 500, "FICSIT-0001", "Compatibility update failed.");
  }
}
