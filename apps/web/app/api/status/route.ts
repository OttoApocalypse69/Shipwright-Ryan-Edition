import { configuredProviderIds } from "@/lib/auth-config";
import { database } from "@/lib/db";
import { publicKeyRawBase64 } from "@/lib/signing";

export async function GET() {
  const checkedAt = new Date().toISOString();
  let databaseStatus: "healthy" | "unavailable" = "unavailable";
  try { await database().query("SELECT 1"); databaseStatus = "healthy"; } catch { /* status pages must not expose database errors */ }
  let signingStatus: "configured" | "unavailable" = "unavailable";
  try { publicKeyRawBase64(); signingStatus = "configured"; } catch { /* fail closed */ }
  return Response.json({
    checkedAt,
    services: {
      controlPlane: databaseStatus,
      authentication: configuredProviderIds().length > 0 ? "configured" : "unavailable",
      entitlementSigning: signingStatus,
      achievementSynchronization: databaseStatus,
      releaseMetadata: process.env.FTEP_GITHUB_REPOSITORY ? "configured" : "unavailable",
    },
  }, { headers: { "Cache-Control": "public, max-age=30, stale-while-revalidate=120" } });
}
