import { auth, signIn } from "@/auth";
import { ConnectionClient } from "./connection-client";
import { accord, accordDocumentHash } from "@/lib/accord";
import { configuredProviderIds } from "@/lib/auth-config";
import { z } from "zod";

export const metadata = { title: "Connect SRE" };

const querySchema = z.object({
  deviceId: z.string().uuid(),
  devicePublicKey: z.string().regex(/^[A-Za-z0-9_-]{43}$/),
  callback: z.string().url().refine((value) => { const url = new URL(value); return url.protocol === "http:" && url.hostname === "127.0.0.1" && Boolean(url.port) && url.pathname === "/callback"; }),
  state: z.string().uuid(),
  accordVersion: z.literal(accord.version),
  accordHash: z.literal(accordDocumentHash),
});

export default async function ConnectPage({ searchParams }: { searchParams: Promise<Record<string, string | string[] | undefined>> }) {
  const raw = await searchParams;
  const parsed = querySchema.safeParse(Object.fromEntries(Object.entries(raw).filter(([, value]) => typeof value === "string")));
  if (!parsed.success) return <section className="shell section"><span className="eyebrow">FTEP connection</span><h2>Invalid SRE request</h2><article className="panel"><p className="muted">Return to SRE and start the browser connection again.</p></article></section>;
  const session = await auth();
  const redirectTo = `/connect?${new URLSearchParams(parsed.data).toString()}`;
  const providers = configuredProviderIds();
  return <section className="shell section"><span className="eyebrow">FTEP connection</span><h2>Connect SRE</h2>{session?.user ? <ConnectionClient {...parsed.data} /> : <article className="panel"><h3>Sign in in your browser</h3><p className="muted">Authentication stays in this browser. No OAuth token is pasted into SRE.</p><div className="actions">{providers.map((provider) => <form key={provider} action={async () => { "use server"; await signIn(provider, { redirectTo }); }}><button className="button" type="submit">Continue with {provider.toUpperCase()}</button></form>)}</div>{providers.length === 0 && <p className="badge">OAUTH CREDENTIALS REQUIRED FOR DEPLOYMENT</p>}</article>}</section>;
}
