import type { PoolClient } from "pg";
import { recordAudit, type ControlPlaneAccount } from "@/lib/control-plane";

export const achievementDefinitions = [
  { id: "FTEP-ACH-0001", title: "Ahh, Zelda", description: "Hope you had fun playing. Now onto Satisfactory.", category: "Zelda", score: 100, hidden: false },
  { id: "FTEP-ACH-0002", title: "Treaty Ratified", description: "You actually agreed to this.", category: "Treaty", score: 10, hidden: false },
  { id: "FTEP-ACH-0003", title: "Somehow This Needed OAuth", description: "You wanted Zelda. We built identity infrastructure.", category: "Infrastructure", score: 10, hidden: false },
  { id: "FTEP-ACH-0004", title: "Legally Supplied Bits", description: "FTEP has detected a completely user-provided collection of data.", category: "Zelda", score: 10, hidden: false },
  { id: "FTEP-ACH-0005", title: "Registered Gaming Apparatus", description: "Your computer is now recognized by the Interpersonal Treaty Authority.", category: "Infrastructure", score: 10, hidden: false },
  { id: "FTEP-ACH-0006", title: "The Invoice Has Come Due", description: "Article II would like a word.", category: "Satisfactory", score: 10, hidden: false },
  { id: "FTEP-ACH-0007", title: "FICSIT Employee Onboarding", description: "Welcome to the factory. Your free time has been processed.", category: "Satisfactory", score: 25, hidden: false },
  { id: "FTEP-ACH-0008", title: "Article II Enjoyer", description: "Industrial cooperation has improved diplomatic relations.", category: "Satisfactory", score: 50, hidden: false },
  { id: "FTEP-ACH-0009", title: "Diplomatic Relations Restored", description: "Zelda privileges restored. Try not to ruin this.", category: "Diplomacy", score: 25, hidden: false },
  { id: "FTEP-ACH-0010", title: "PostgreSQL Was Necessary", description: "It absolutely was not.", category: "Infrastructure", score: 10, hidden: false },
  { id: "FTEP-ACH-0011", title: "Enterprise Gaming", description: "A distributed system was deployed so two people could play video games.", category: "Infrastructure", score: 100, hidden: false },
  { id: "FTEP-ACH-0012", title: "Ryan Moment", description: "Engineering could not have reasonably anticipated this.", category: "Administrative", score: 50, hidden: false },
  { id: "FTEP-ACH-0013", title: "Fluid Logistics", description: "Biological machinery also requires input buffers.", category: "Satisfactory", score: 10, hidden: false },
  { id: "FTEP-ACH-0014", title: "Pipeline Operational", description: "Water successfully delivered to operator.", category: "Satisfactory", score: 25, hidden: false },
  { id: "FTEP-ACH-0015", title: "That Is Not Zelda", description: "Animal Crossing support has been requested under the Zelda modernization programme.", category: "Expansion", score: 10, hidden: false },
] as const;

export type AchievementEventInput = {
  eventId: string;
  eventType: string;
  deviceId?: string;
  occurredAt: Date;
  payload: Record<string, unknown>;
  source: "SRE" | "FTEP" | "ADMIN";
  schemaVersion: number;
};

const achievementForId = new Set<string>(achievementDefinitions.map((definition) => definition.id));

function triggeredAchievements(event: AchievementEventInput, hydrationCount: number): string[] {
  const gameId = typeof event.payload.gameId === "string" ? event.payload.gameId : "";
  switch (event.eventType) {
    case "GAMEPLAY_READY":
      return gameId.startsWith("zelda-") ? ["FTEP-ACH-0001"] : [];
    case "GAME_LAUNCHED":
      return gameId === "animal-crossing-new-horizons" ? ["FTEP-ACH-0015"] : [];
    case "TREATY_ACCEPTED":
      return ["FTEP-ACH-0002"];
    case "ACCOUNT_LOGIN_SUCCEEDED":
      return ["FTEP-ACH-0003"];
    case "GAME_DATA_VALIDATED":
      return ["FTEP-ACH-0004"];
    case "DEVICE_REGISTERED":
      return ["FTEP-ACH-0005"];
    case "ARTICLE_II_ACTIVATED":
      return ["FTEP-ACH-0006"];
    case "SATISFACTORY_SESSION_QUALIFIED":
      return ["FTEP-ACH-0007", "FTEP-ACH-0008"];
    case "TREATY_STATE_CHANGED":
      return event.payload.to === "COMPLIANT" || event.payload.to === "RESTORED" ? ["FTEP-ACH-0009"] : [];
    case "RYAN_MOMENT_CLASSIFIED":
      return event.payload.code === "FICSIT-0010" ? ["FTEP-ACH-0012"] : [];
    case "HYDRATION_ACKNOWLEDGED":
      return hydrationCount >= 5 ? ["FTEP-ACH-0013", "FTEP-ACH-0014"] : ["FTEP-ACH-0013"];
    default:
      return [];
  }
}

export function sanitizeEventPayload(payload: Record<string, unknown>): Record<string, unknown> {
  let serialized: string;
  try {
    serialized = JSON.stringify(payload);
  } catch {
    throw new Error("FTEP_EVENT_PAYLOAD_NOT_ALLOWED");
  }
  if (!serialized) throw new Error("FTEP_EVENT_PAYLOAD_NOT_ALLOWED");
  if (Buffer.byteLength(serialized, "utf8") > 4096) throw new Error("FTEP_EVENT_TOO_LARGE");
  const forbidden = /path|token|secret|password|private|save|rom|firmware|key|process|screenshot|binary/i;
  const inspect = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) inspect(item);
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
      if (forbidden.test(key)) throw new Error("FTEP_EVENT_PAYLOAD_NOT_ALLOWED");
      inspect(entry);
    }
  };
  inspect(payload);
  return payload;
}

export async function ingestAchievementEvent(
  client: PoolClient,
  account: ControlPlaneAccount,
  event: AchievementEventInput,
): Promise<{ accepted: boolean; alreadyProcessed: boolean; unlocked: string[] }> {
  sanitizeEventPayload(event.payload);
  const inserted = await client.query<{ event_id: string }>(
    `INSERT INTO achievement_events(event_id, user_id, device_id, event_type, occurred_at, source, schema_version, payload)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb)
     ON CONFLICT(event_id) DO NOTHING
     RETURNING event_id`,
    [event.eventId, account.id, event.deviceId ?? null, event.eventType, event.occurredAt, event.source, event.schemaVersion, JSON.stringify(event.payload)],
  );
  if (inserted.rowCount !== 1) {
    const existingEvent = await client.query<{ user_id: string }>("SELECT user_id FROM achievement_events WHERE event_id = $1", [event.eventId]);
    if (existingEvent.rows[0]?.user_id !== account.id) throw new Error("FTEP_EVENT_ID_CONFLICT");
    const existing = await client.query<{ achievement_id: string }>("SELECT achievement_id FROM achievement_unlocks WHERE user_id = $1 AND client_event_id LIKE $2", [account.id, `${event.eventId}:%`]);
    return { accepted: true, alreadyProcessed: true, unlocked: existing.rows.map((row) => row.achievement_id) };
  }
  const hydrationCount = event.eventType === "HYDRATION_ACKNOWLEDGED"
    ? Number((await client.query<{ count: string }>("SELECT COUNT(*)::text AS count FROM achievement_events WHERE user_id = $1 AND event_type = 'HYDRATION_ACKNOWLEDGED'", [account.id])).rows[0]?.count ?? "1")
    : 0;
  const candidateIds = triggeredAchievements(event, hydrationCount).filter((id) => achievementForId.has(id));
  const unlocked: string[] = [];
  const sourceSessionId = typeof event.payload.sessionId === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(event.payload.sessionId)
    ? event.payload.sessionId
    : null;
  for (const achievementId of candidateIds) {
    const result = await client.query(
      `INSERT INTO achievement_unlocks(user_id, achievement_id, unlocked_at, source_session_id, client_event_id)
       VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT(user_id, achievement_id) DO NOTHING`,
      [account.id, achievementId, event.occurredAt, sourceSessionId, `${event.eventId}:${achievementId}`],
    );
    if (result.rowCount === 1) unlocked.push(achievementId);
  }
  await recordAudit(client, account.id, "achievement.event.ingest", "achievement_event", event.eventId, {
    eventType: event.eventType,
    source: event.source,
    unlocked,
  });
  return { accepted: true, alreadyProcessed: false, unlocked };
}
