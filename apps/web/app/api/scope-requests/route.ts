import { auth } from "@/auth";
import { withAccountClient, withAccountTransaction, recordAudit } from "@/lib/control-plane";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { z } from "zod";

const schema = z.object({ gameTitle: z.string().trim().min(2).max(160), notes: z.string().trim().max(2000).optional() });

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const requests = await withAccountClient(session.user, async (client, account) => {
      const rows = await client.query(`SELECT id, game_title AS "gameTitle", state, notes, created_at AS "createdAt", updated_at AS "updatedAt" FROM scope_requests WHERE requested_by = $1 ORDER BY created_at DESC`, [account.id]);
      return rows.rows;
    });
    return jsonOk(request, { requests });
  } catch (error) {
    console.error("FTEP scope request lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load scope requests.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try { input = schema.parse(await requestJson(request)); } catch { return jsonError(request, 400, "FICSIT-0001", "Invalid scope request."); }
  try {
    const result = await withAccountTransaction(session.user, async (client, account) => {
      const created = await client.query<{ id: string }>("INSERT INTO scope_requests(requested_by, game_title, state, notes) VALUES ($1, $2, 'REQUESTED', $3) RETURNING id", [account.id, input.gameTitle, input.notes ?? null]);
      const id = created.rows[0]?.id;
      if (!id) throw new Error("FTEP_SCOPE_REQUEST_FAILED");
      await recordAudit(client, account.id, "scope.request", "scope_request", id, { gameTitle: input.gameTitle });
      return { id, state: "REQUESTED" as const };
    });
    return jsonOk(request, result, { status: 201 });
  } catch (error) {
    console.error("FTEP scope request creation failed", error);
    return jsonError(request, 500, "FICSIT-0001", "Scope request could not be recorded.");
  }
}
