import { compatibilityRows } from "@/lib/catalog";

export function GET() {
  return Response.json({ schemaVersion: 1, compatibility: compatibilityRows() }, { headers: { "Cache-Control": "public, s-maxage=300, stale-while-revalidate=600" } });
}
