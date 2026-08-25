import { publicKeyRawBase64 } from "@/lib/signing";

export function GET() {
  try { return Response.json({ algorithm: "Ed25519", keyId: process.env.FTEP_SIGNING_KEY_ID, publicKey: publicKeyRawBase64() }, { headers: { "Cache-Control": "public, max-age=300" } }); }
  catch (error) {
    console.error("FTEP signing key unavailable", error instanceof Error ? error.message : error);
    return Response.json({ error: "Signing key is not configured." }, { status: 503 });
  }
}
