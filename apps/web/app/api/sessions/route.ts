import { auth } from "@/auth";
import { ingestAchievementEvent } from "@/lib/achievement-service";
import { withAccountClient, withAccountTransaction, recordAudit } from "@/lib/control-plane";
import { apiIdentity, readRequestBody } from "@/lib/device-auth";
import { jsonError, jsonOk } from "@/lib/http";
import { gameById } from "@/lib/catalog";
import { z } from "zod";

const states = ["REQUESTED", "AUTHORIZING", "PREPARING", "STARTING_RUNTIME", "RUNTIME_STARTED", "PLAYABLE", "ENDING", "ENDED", "FAILED"] as const;
const sessionSchema = z.object({
  sessionId: z.string().uuid(),
  clientEventId: z.string().trim().min(8).max(160).regex(/^[A-Za-z0-9._:-]+$/).optional(),
  deviceId: z.string().uuid(),
  gameId: z.string().trim().min(2).max(100),
  variantId: z.string().trim().min(1).max(80),
  runtimeId: z.string().trim().min(1).max(80),
  state: z.enum(states),
  requestedAt: z.union([z.string().datetime({ offset: true }), z.number().int().positive()]).optional(),
  startedAt: z.union([z.string().datetime({ offset: true }), z.number().int().positive()]).optional(),
  playableAt: z.union([z.string().datetime({ offset: true }), z.number().int().positive()]).optional(),
  endedAt: z.union([z.string().datetime({ offset: true }), z.number().int().positive()]).optional(),
  durationMs: z.number().int().min(0).max(7 * 24 * 60 * 60 * 1000).optional(),
  playablePrecision: z.enum(["PLAYABLE_EXACT", "PLAYABLE_APPROXIMATE"]).optional(),
  playableMethod: z.string().trim().max(120).optional(),
  exitCode: z.number().int().min(-32_768).max(32_767).optional(),
  launchResult: z.string().trim().min(1).max(120),
  qualifying: z.boolean().optional(),
  activeMinutes: z.number().int().min(0).max(24 * 60).optional(),
  note: z.string().trim().max(1000).optional(),
});

const rank: Record<(typeof states)[number], number> = {
  REQUESTED: 0, AUTHORIZING: 1, PREPARING: 2, STARTING_RUNTIME: 3, RUNTIME_STARTED: 4,
  PLAYABLE: 5, ENDING: 6, ENDED: 7, FAILED: 99,
};

function iso(value: string | number | undefined): Date | null { return value === undefined ? null : new Date(value); }
function validGame(gameId: string): boolean { return gameId === "satisfactory" || Boolean(gameById(gameId)); }

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  const url = new URL(request.url);
  const limit = Math.min(Math.max(Number.parseInt(url.searchParams.get("limit") ?? "50", 10) || 50, 1), 100);
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      const rows = await client.query(
        `SELECT s.id AS "sessionId", s.state, s.requested_at AS "requestedAt", s.started_at AS "startedAt",
                s.playable_at AS "playableAt", s.ended_at AS "endedAt", s.duration_ms AS "durationMs",
                s.playable_precision AS "playablePrecision", s.playable_method AS "playableMethod",
                s.exit_code AS "exitCode", s.launch_result AS "launchResult", s.reported_at AS "reportedAt",
                g.id AS "gameId", g.title, gv.variant_key AS "variantId", s.runtime_id AS "runtimeId",
                d.id AS "deviceId", d.label AS "deviceLabel"
           FROM game_sessions s
           JOIN game_variants gv ON gv.id = s.game_variant_id
           JOIN games g ON g.id = gv.game_id
           JOIN devices d ON d.id = s.device_id
          WHERE s.user_id = $1
          ORDER BY s.requested_at DESC LIMIT $2`,
        [account.id, limit],
      );
      return rows.rows;
    });
    return jsonOk(request, { sessions: result });
  } catch (error) {
    console.error("FTEP session lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load sessions.");
  }
}

export async function POST(request: Request) {
  try {
    const parsedBody = await readRequestBody(request);
    const identity = await apiIdentity(request, parsedBody.body);
    if (!identity) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
    const input = sessionSchema.parse(parsedBody.json);
    if (identity.via === "DEVICE_SIGNATURE" && identity.deviceId !== input.deviceId) return jsonError(request, 403, "FICSIT-0004", "The session device does not match the authenticated device.");
    if (!validGame(input.gameId)) return jsonError(request, 400, "FICSIT-0007", "This game is not in the FTEP catalog.");
    const result = await withAccountTransaction(identity, async (client, account) => {
      const device = await client.query("SELECT 1 FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL FOR UPDATE", [input.deviceId, account.id]);
      if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_REGISTERED");
      const variant = await client.query<{ id: string }>("SELECT id FROM game_variants WHERE game_id = $1 AND variant_key = $2", [input.gameId, input.variantId]);
      if (variant.rowCount !== 1) throw new Error("FTEP_VARIANT_NOT_FOUND");
      const compatibility = await client.query<{ status: string }>(
        "SELECT status FROM runtime_compatibility WHERE game_variant_id = $1 AND runtime_id = $2 ORDER BY CASE WHEN version_range = '*' THEN 0 ELSE 1 END LIMIT 1",
        [variant.rows[0].id, input.runtimeId],
      );
      if (compatibility.rowCount !== 1 || ["UNSUPPORTED", "BROKEN", "DEPRECATED"].includes(compatibility.rows[0]?.status ?? "UNSUPPORTED")) throw new Error("FTEP_RUNTIME_NOT_COMPATIBLE");
      const existing = await client.query<{ user_id: string; state: (typeof states)[number] }>("SELECT user_id, state FROM game_sessions WHERE id = $1 FOR UPDATE", [input.sessionId]);
      if (existing.rows[0]?.user_id && existing.rows[0].user_id !== account.id) throw new Error("FTEP_SESSION_OWNERSHIP_CONFLICT");
      if (existing.rows[0] && existing.rows[0].state !== input.state && rank[input.state] < rank[existing.rows[0].state] && input.state !== "FAILED") throw new Error("FTEP_SESSION_STATE_REGRESSION");
      await client.query(
        `INSERT INTO game_sessions(id, user_id, device_id, game_variant_id, runtime_id, state, requested_at, started_at, playable_at, ended_at, duration_ms, playable_precision, playable_method, exit_code, launch_result, client_event_id, reported_at, correction_note)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, now(), $17)
         ON CONFLICT(id) DO UPDATE SET
           state = EXCLUDED.state, started_at = COALESCE(EXCLUDED.started_at, game_sessions.started_at), playable_at = COALESCE(EXCLUDED.playable_at, game_sessions.playable_at),
           ended_at = COALESCE(EXCLUDED.ended_at, game_sessions.ended_at), duration_ms = COALESCE(EXCLUDED.duration_ms, game_sessions.duration_ms),
           playable_precision = COALESCE(EXCLUDED.playable_precision, game_sessions.playable_precision), playable_method = COALESCE(EXCLUDED.playable_method, game_sessions.playable_method),
           exit_code = COALESCE(EXCLUDED.exit_code, game_sessions.exit_code), launch_result = EXCLUDED.launch_result, reported_at = now(),
           correction_note = COALESCE(EXCLUDED.correction_note, game_sessions.correction_note)
         WHERE game_sessions.user_id = EXCLUDED.user_id`,
        [input.sessionId, account.id, input.deviceId, variant.rows[0].id, input.runtimeId, input.state, iso(input.requestedAt) ?? new Date(), iso(input.startedAt), iso(input.playableAt), iso(input.endedAt), input.durationMs ?? null, input.playablePrecision ?? null, input.playableMethod ?? null, input.exitCode ?? null, input.launchResult, input.clientEventId ?? input.sessionId, input.note ?? null],
      );
      const eventResults: Array<{ unlocked: string[] }> = [];
      if (input.state === "PLAYABLE") {
        eventResults.push(await ingestAchievementEvent(client, account, {
          eventId: `session:${input.sessionId}:playable`, eventType: "GAMEPLAY_READY", deviceId: input.deviceId,
          occurredAt: iso(input.playableAt) ?? new Date(), payload: { gameId: input.gameId, sessionId: input.sessionId, precision: input.playablePrecision ?? "PLAYABLE_APPROXIMATE" }, source: "SRE", schemaVersion: 1,
        }));
      }
      if (input.gameId === "satisfactory" && input.qualifying && (input.activeMinutes ?? 0) >= 30) {
        eventResults.push(await ingestAchievementEvent(client, account, {
          eventId: `session:${input.sessionId}:qualified`, eventType: "SATISFACTORY_SESSION_QUALIFIED", deviceId: input.deviceId,
          occurredAt: iso(input.endedAt) ?? new Date(), payload: { gameId: "satisfactory", sessionId: input.sessionId, activeMinutes: input.activeMinutes }, source: "SRE", schemaVersion: 1,
        }));
      }
      await recordAudit(client, account.id, existing.rows[0] ? "session.correct" : "session.report", "game_session", input.sessionId, { gameId: input.gameId, state: input.state, runtimeId: input.runtimeId, qualifying: input.qualifying ?? false });
      return { sessionId: input.sessionId, state: input.state, synchronized: true as const, achievements: eventResults.flatMap((item) => item.unlocked) };
    });
    return jsonOk(request, result, { status: 202 });
  } catch (error) {
    if (error instanceof z.ZodError) return jsonError(request, 400, "FICSIT-0001", "Invalid session report.");
    if (error instanceof SyntaxError) return jsonError(request, 400, "FICSIT-0001", "Invalid JSON request.");
    if (error instanceof Error && error.message === "FTEP_REQUEST_TOO_LARGE") return jsonError(request, 413, "FICSIT-0001", "Request is too large.");
    if (error instanceof Error && ["FTEP_DEVICE_REQUEST_EXPIRED", "FTEP_DEVICE_REQUEST_REPLAYED", "FTEP_DEVICE_SIGNATURE_INVALID", "FTEP_DEVICE_NOT_REGISTERED"].includes(error.message)) return jsonError(request, 401, "FICSIT-0003", "Device authentication failed.");
    if (error instanceof Error && error.message === "FTEP_VARIANT_NOT_FOUND") return jsonError(request, 400, "FICSIT-0007", "That game variant is not supported by the FTEP catalog.");
    if (error instanceof Error && error.message === "FTEP_RUNTIME_NOT_COMPATIBLE") return jsonError(request, 400, "FICSIT-0007", "That runtime is not currently compatible with this game variant.");
    if (error instanceof Error && error.message === "FTEP_SESSION_OWNERSHIP_CONFLICT") return jsonError(request, 409, "FICSIT-0001", "That session belongs to another account.");
    if (error instanceof Error && error.message === "FTEP_SESSION_STATE_REGRESSION") return jsonError(request, 409, "FICSIT-0001", "Session state cannot move backwards.");
    console.error("FTEP session report failed", error);
    return jsonError(request, 500, "FICSIT-0001", "Session synchronization failed.");
  }
}
