import { auth } from "@/auth";
import { ingestAchievementEvent } from "@/lib/achievement-service";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { accord, accordDocumentHash } from "@/lib/accord";
import { withAccountClient, withAccountTransaction, recordAudit } from "@/lib/control-plane";
import { database } from "@/lib/db";
import { jsonError, jsonOk, requestJson } from "@/lib/http";
import { isLocalDatabaseUnavailable } from "@/lib/local-credentials";
import { readLocalFtepState, ratifyLocalDevice } from "@/lib/local-ftep-state";
import { z } from "zod";

const schema = z.object({
  deviceId: z.string().uuid(),
  version: z.literal(accord.version),
  documentHash: z.literal(accordDocumentHash),
});

export async function GET(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  try {
    const result = await withAccountClient(session.user, async (client, account) => {
      const ratification = await client.query(
        `SELECT t.id AS "treatyId", tv.semantic_version AS version, r.document_hash AS "documentHash", r.accepted_at AS "acceptedAt"
           FROM ratifications r
           JOIN treaty_versions tv ON tv.id = r.treaty_version_id
           JOIN treaties t ON t.id = tv.treaty_id
          WHERE r.user_id = $1
          ORDER BY r.accepted_at DESC
          LIMIT 1`,
        [account.id],
      );
      const compliance = await client.query(
        `SELECT state, details, recorded_at AS "recordedAt"
           FROM compliance_events WHERE user_id = $1 ORDER BY recorded_at DESC LIMIT 1`,
        [account.id],
      );
      const entitlement = await client.query(
        `SELECT entitlement_key AS "key", state, granted_at AS "grantedAt"
           FROM entitlements WHERE user_id = $1 ORDER BY entitlement_key`,
        [account.id],
      );
      return {
        treaty: ratification.rows[0] ?? null,
        compliance: compliance.rows[0] ?? { state: "PENDING_ACCEPTANCE", details: {}, recordedAt: null },
        entitlements: entitlement.rows,
      };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      const state = await readLocalFtepState();
      const email = session.user.email.trim().toLowerCase();
      const ratification = state.ratifications.filter((item) => item.email === email).sort((left, right) => right.acceptedAtUnixMs - left.acceptedAtUnixMs)[0];
      const entitlements = state.entitlements.filter((item) => item.email === email).map((item) => ({ key: item.key, state: item.state, grantedAt: new Date(item.grantedAtUnixMs).toISOString() }));
      return jsonOk(request, {
        treaty: ratification ? { treatyId: accord.treatyId, version: ratification.version, documentHash: ratification.documentHash, acceptedAt: new Date(ratification.acceptedAtUnixMs).toISOString() } : null,
        compliance: { state: ratification ? "COMPLIANT" : "PENDING_ACCEPTANCE", details: {}, recordedAt: ratification ? new Date(ratification.acceptedAtUnixMs).toISOString() : null },
        entitlements,
      }, { headers: { "X-FTEP-Storage": "local-development" } });
    }
    console.error("FTEP ratification lookup failed", error);
    return jsonError(request, 500, "FICSIT-0001", "FTEP could not load treaty status.");
  }
}

export async function POST(request: Request) {
  const session = await auth();
  if (!session?.user.email) return jsonError(request, 401, "FICSIT-0003", "Authentication required.");
  let input: z.infer<typeof schema>;
  try {
    input = schema.parse(await requestJson(request));
  } catch {
    return jsonError(request, 400, "FICSIT-0005", "The Accord identity is invalid.");
  }
  try {
    const result = await withAccountTransaction(session.user, async (client, account) => {
      const device = await client.query("SELECT 1 FROM devices WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL FOR UPDATE", [input.deviceId, account.id]);
      if (device.rowCount !== 1) throw new Error("FTEP_DEVICE_NOT_REGISTERED");
      const treatyVersion = await client.query<{ active_version_id: string | null }>(
        `INSERT INTO treaties(id, title) VALUES ($1, $2)
         ON CONFLICT(id) DO UPDATE SET title = EXCLUDED.title
         RETURNING active_version_id`,
        [accord.treatyId, accord.title],
      );
      const version = await client.query<{ id: string }>(
        `INSERT INTO treaty_versions(treaty_id, semantic_version, document_hash, structured_document, published_at)
         VALUES ($1, $2, $3, $4::jsonb, now())
         ON CONFLICT(treaty_id, semantic_version) DO UPDATE SET document_hash = EXCLUDED.document_hash, structured_document = EXCLUDED.structured_document
         RETURNING id`,
        [accord.treatyId, accord.version, accordDocumentHash, JSON.stringify(accord)],
      );
      const versionId = version.rows[0]?.id ?? treatyVersion.rows[0]?.active_version_id;
      if (!versionId) throw new Error("FTEP_TREATY_UNAVAILABLE");
      await client.query("UPDATE treaties SET active_version_id = $1 WHERE id = $2", [versionId, accord.treatyId]);
      for (const article of accord.articles) {
        await client.query(
          `INSERT INTO treaty_articles(treaty_version_id, article_number, title, body)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT(treaty_version_id, article_number) DO UPDATE SET title = EXCLUDED.title, body = EXCLUDED.body`,
          [versionId, article.number, article.title, article.summary],
        );
      }
      await client.query(
        `INSERT INTO ratifications(treaty_version_id, user_id, accepted_at, document_hash)
         VALUES ($1, $2, now(), $3)
         ON CONFLICT(treaty_version_id, user_id) DO UPDATE SET accepted_at = EXCLUDED.accepted_at, document_hash = EXCLUDED.document_hash`,
        [versionId, account.id, accordDocumentHash],
      );
      await client.query(
        `INSERT INTO entitlements(user_id, entitlement_key, state)
         VALUES ($1, 'ftep.library.nintendo', 'ACTIVE')
         ON CONFLICT(user_id, entitlement_key) DO UPDATE SET state = 'ACTIVE', granted_at = now()`,
        [account.id],
      );
      await client.query(
        `INSERT INTO compliance_events(user_id, state, details, recorded_by)
         VALUES ($1, 'COMPLIANT', $2::jsonb, $1)`,
        [account.id, JSON.stringify({ treatyId: accord.treatyId, version: accord.version, documentHash: accordDocumentHash })],
      );
      await recordAudit(client, account.id, "treaty.ratify", "treaty", accord.treatyId, {
        version: accord.version,
        documentHash: accordDocumentHash,
        deviceId: input.deviceId,
      });
      await ingestAchievementEvent(client, account, {
        eventId: `treaty:${accord.treatyId}:${accord.version}:${account.id}`,
        eventType: "TREATY_ACCEPTED",
        deviceId: input.deviceId,
        occurredAt: new Date(),
        payload: { treatyId: accord.treatyId, version: accord.version },
        source: "FTEP",
        schemaVersion: 1,
      });
      return { treatyId: accord.treatyId, version: accord.version, documentHash: accordDocumentHash, ratified: true as const };
    });
    return jsonOk(request, result);
  } catch (error) {
    if (error instanceof Error && error.message === "FTEP_DEVICE_NOT_REGISTERED") return jsonError(request, 403, "FICSIT-0004", "Register this SRE device before ratifying the Accord.");
    if (localCredentialsEnabled() && isLocalDatabaseUnavailable(error)) {
      try {
        const result = await ratifyLocalDevice({ email: session.user.email, deviceId: input.deviceId, version: input.version, documentHash: input.documentHash });
        return jsonOk(request, result, { headers: { "X-FTEP-Storage": "local-development" } });
      } catch (localError) {
        console.error("FTEP local Accord ratification failed", localError);
        return jsonError(request, 500, "FICSIT-0005", "Local Accord ratification failed.");
      }
    }
    console.error("FTEP Accord ratification failed", error);
    return jsonError(request, 500, "FICSIT-0005", "Accord ratification failed.");
  }
}
