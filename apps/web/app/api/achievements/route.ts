import { auth } from "@/auth";
import { achievementDefinitions } from "@/lib/achievement-service";
import { withAccountClient } from "@/lib/control-plane";
import { jsonError, jsonOk } from "@/lib/http";

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      const unlocks = await client.query<{ achievement_id: string; unlocked_at: string }>(
        `SELECT achievement_id, unlocked_at FROM achievement_unlocks WHERE user_id = $1 ORDER BY unlocked_at DESC`,
        [account.id],
      );
      const unlockedAt = new Map(unlocks.rows.map((row) => [row.achievement_id, row.unlocked_at]));
      return {
        total: achievementDefinitions.length,
        unlocked: unlocks.rows.length,
        achievements: achievementDefinitions.map((definition) => ({
          ...definition,
          unlockedAt: unlockedAt.get(definition.id) ?? null,
          locked: !unlockedAt.has(definition.id),
        })),
        lastSynchronizedAt: unlocks.rows[0]?.unlocked_at ?? null,
      };
    });
    return jsonOk(request, result);
  } catch (error) {
    console.error("FTEP achievement lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load achievements.");
  }
}
