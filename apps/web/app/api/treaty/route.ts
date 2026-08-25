import { accord, accordCanonicalJson, accordDocumentHash } from "@/lib/accord";

export function GET() {
  return Response.json(
    {
      treatyId: accord.treatyId,
      version: accord.version,
      title: accord.title,
      documentHash: accordDocumentHash,
      canonicalDocument: accordCanonicalJson,
      schedule: accord.schedule,
      articles: accord.articles,
    },
    { headers: { "Cache-Control": "public, max-age=300, stale-while-revalidate=900" } },
  );
}
