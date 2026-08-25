import { ingestAchievementEvent } from "@/lib/achievement-service";
import { withAccountTransaction } from "@/lib/control-plane";
import { apiIdentity, readRequestBody } from "@/lib/device-auth";
import { jsonError, jsonOk } from "@/lib/http";
import { z } from "zod";

const eventSchema = z.object({
  eventId: z.string().trim().min(8).max(160).regex(/^[A-Za-z0-9._:-]+$/),
  eventType: z.enum([
    "ACCOUNT_LOGIN_SUCCEEDED",
    "TREATY_ACCEPTED",
    "GAME_DATA_VALIDATED",
    "DEVICE_REGISTERED",
    "GAMEPLAY_READY",
    "GAME_LAUNCHED",
    "ARTICLE_II_ACTIVATED",
    "SATISFACTORY_SESSION_QUALIFIED",
    "TREATY_STATE_CHANGED",
    "RYAN_MOMENT_CLASSIFIED",
    "HYDRATION_ACKNOWLEDGED",
  ]),
  deviceId: z.string().uuid().optional(),
  occurredAt: z.union([z.string().datetime({ offset: true }), z.number().int().positive()]),
  schemaVersion: z.number().int().positive().max(10).default(1),
  payload: z.record(z.string(), z.unknown()).default({}),
});

export async function POST(request: Request) {
  let body: string;
  let identity: Awaited<ReturnType<typeof apiIdentity>>;
  try {
    const parsedBody = await readRequestBody(request);
    body = parsedBody.body;
    identity = await apiIdentity(request, body);
    if (!identity) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
    const input = eventSchema.parse(parsedBody.json);
    const deviceId = input.deviceId ?? identity.deviceId;
    if (identity.via === "DEVICE_SIGNATURE" && deviceId !== identity.deviceId) return jsonError(request, 403, "FICSIT-0004", "The event device does not match the authenticated device.");
    if (!deviceId) return jsonError(request, 400, "FICSIT-0004", "A registered device is required for synchronization.");
    return await ingest(request, identity, { ...input, deviceId });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_REQUEST_TOO_LARGE") return jsonError(request, 413, "FICSIT-0001", "Request is too large.");
    if (error instanceof Error && ["FTEP_DEVICE_REQUEST_EXPIRED", "FTEP_DEVICE_REQUEST_REPLAYED", "FTEP_DEVICE_SIGNATURE_INVALID", "FTEP_DEVICE_NOT_REGISTERED"].includes(error.message)) return jsonError(request, 401, "FICSIT-0003", "Device authentication failed.");
    if (error instanceof SyntaxError) return jsonError(request, 400, "FICSIT-0001", "Invalid JSON request.");
    if (error instanceof z.ZodError) return jsonError(request, 400, "FICSIT-0001", "Invalid achievement event.");
    console.error("FTEP achievement event ingestion failed", error);
    return jsonError(request, 500, "FICSIT-0001", "Achievement synchronization failed.");
  }
}

async function ingest(request: Request, identity: NonNullable<Awaited<ReturnType<typeof apiIdentity>>>, input: z.infer<typeof eventSchema>) {
  const occurredAt = new Date(typeof input.occurredAt === "number" ? input.occurredAt : input.occurredAt);
  const now = Date.now();
  if (occurredAt.getTime() > now + 5 * 60_000 || occurredAt.getTime() < now - 366 * 24 * 60 * 60_000) {
    return jsonError(request, 400, "FICSIT-0001", "Achievement event timestamp is outside the accepted window.");
  }
  try {
    const result = await withAccountTransaction(identity, async (client, account) => {
      if (input.deviceId) {
        const device = await client.query("SELECT 1 FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL", [input.deviceId, account.id]);
        if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_REGISTERED");
      }
      return await ingestAchievementEvent(client, account, {
        eventId: input.eventId,
        eventType: input.eventType,
        deviceId: input.deviceId,
        occurredAt,
        payload: input.payload,
        source: "SRE",
        schemaVersion: input.schemaVersion,
      });
    });
    return jsonOk(request, result, { status: result.alreadyProcessed ? 200 : 202 });
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_DEVICE_NOT_REGISTERED") return jsonError(request, 403, "FICSIT-0004", "The reporting device is not registered to this account.");
    if (error instanceof Error && error.message === "FTEP_EVENT_ID_CONFLICT") return jsonError(request, 409, "FICSIT-0001", "This event identifier belongs to another account.");
    if (error instanceof Error && error.message === "FTEP_EVENT_PAYLOAD_NOT_ALLOWED") return jsonError(request, 400, "FICSIT-0001", "This event contains data outside the privacy boundary.");
    if (error instanceof Error && error.message === "FTEP_EVENT_TOO_LARGE") return jsonError(request, 413, "FICSIT-0001", "Achievement event is too large.");
    throw error;
  }
}
