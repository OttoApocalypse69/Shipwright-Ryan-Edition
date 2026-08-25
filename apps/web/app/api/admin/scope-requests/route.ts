import { auth } from "@/auth";
import { withAccountClient, withAccountTransaction, recordAudit, requireAdminAccount } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const schema = z.object({ id: z.string().uuid(), state: z.enum(["REQUESTED", "UNDER_REVIEW", "ACCEPTED", "IN_PROGRESS", "SUPPORTED", "REJECTED", "DEFERRED"]), notes: z.string().trim().max(2000).optional() });

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const requests = await withAccountClient(session.user, async (client, account) => { requireAdminAccount(account); const rows = await client.query(`SELECT sr.id, sr.game_title AS "gameTitle", sr.state, sr.notes, sr.created_at AS "createdAt", sr.updated_at AS "updatedAt", u.email AS "requestedBy" FROM scope_requests sr JOIN users u ON u.id = sr.requested_by ORDER BY sr.created_at DESC LIMIT 250`); return rows.rows; });
    return jsonOk(request, { requests });
  } catch (error) { if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required."); console.error("FTEP admin scope request lookup failed", error); return jsonError(request, 500, "FICSIT-0001", "FTEP could not load scope requests."); }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try { input = schema.parse(await requestJson(request)); } catch { return jsonError(request, 400, "FICSIT-0001", "Invalid scope request transition."); }
  try {
    const result = await withAccountTransaction(session.user, async (client, account) => { requireAdminAccount(account); const updated = await client.query<{ id: string }>("UPDATE scope_requests SET state = $2, notes = COALESCE($3, notes), updated_at = now() WHERE id = $1 RETURNING id", [input.id, input.state, input.notes ?? null]); if (updated.rowCount !== 1) throw new Error("FTEP_SCOPE_REQUEST_NOT_FOUND"); await recordAudit(client, account.id, "scope.request.update", "scope_request", input.id, { state: input.state }); return { id: input.id, state: input.state }; });
    return jsonOk(request, result);
  } catch (error) { if (error instanceof Error && error.message === "FTEP_ADMIN_REQUIRED") return jsonError(request, 403, "FTEP_ADMIN_REQUIRED", "Administrator role required."); if (error instanceof Error && error.message === "FTEP_SCOPE_REQUEST_NOT_FOUND") return jsonError(request, 404, "FICSIT-0001", "Scope request was not found."); console.error("FTEP admin scope request update failed", error); return jsonError(request, 500, "FICSIT-0001", "Scope request update failed."); }
}
