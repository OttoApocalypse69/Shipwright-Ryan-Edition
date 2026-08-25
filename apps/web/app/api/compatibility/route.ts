import { compatibilityRows } from "@/lib/catalog";
import { database } from "@/lib/db";

export async function GET() {
  try {
    const result = await database().query(
      `SELECT g.id AS "gameId", g.title, gv.variant_key AS variant, gv.original_platform AS platform, rc.runtime_id AS "runtimeId", rc.status, rc.version_range AS "versionRange", rc.notes
         FROM runtime_compatibility rc JOIN game_variants gv ON gv.id = rc.game_variant_id JOIN games g ON g.id = gv.game_id
        WHERE g.active = true ORDER BY g.title, gv.variant_key, rc.runtime_id`,
    );
    if (result.rows.length > 0) return Response.json({ schemaVersion: 1, compatibility: result.rows }, { headers: { "Cache-Control": "public, s-maxage=300, stale-while-revalidate=600" } });
  } catch {
    // The public catalog remains useful during an unavailable database window.
  }
  return Response.json({ schemaVersion: 1, compatibility: compatibilityRows() }, { headers: { "Cache-Control": "public, s-maxage=300, stale-while-revalidate=600" } });
}
