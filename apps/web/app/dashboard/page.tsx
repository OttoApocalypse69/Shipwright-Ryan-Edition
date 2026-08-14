import { auth, signIn, signOut } from "@/auth";
import Link from "next/link";
import { localCredentialsEnabled } from "@/lib/auth-config";

export const metadata = { title: "Dashboard" };
export default async function DashboardPage() {
  const session = await auth(); const githubReady = Boolean(process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET); const localAccounts = localCredentialsEnabled();
  return <section className="shell section"><span className="eyebrow">FTEP account</span><h2>Dashboard</h2>{session?.user ? <article className="panel"><h3>{session.user.name ?? session.user.email}</h3><p className="muted">Role: {session.user.role}. Your devices, sessions, ratification, entitlements, and achievement sync live behind this identity.</p><div className="actions"><Link className="button secondary" href="/devices">Devices</Link><Link className="button secondary" href="/sessions">Sessions</Link><form action={async () => { "use server"; await signOut({ redirectTo: "/" }); }}><button className="button" type="submit">Sign out</button></form></div></article> : <article className="panel"><h3>Browser-based sign in</h3><p className="muted">SRE opens this flow in your browser. It never asks you to paste an access token.</p><div className="actions">{localAccounts && <><Link className="button" href="/sign-in">Sign in to local FTEP</Link><Link className="button secondary" href="/sign-up">Create local account</Link></>}{githubReady && <form action={async () => { "use server"; await signIn("github", { redirectTo: "/dashboard" }); }}><button className="button" type="submit">Continue with GitHub</button></form>}</div>{!localAccounts && !githubReady && <p className="badge">OAUTH CREDENTIALS REQUIRED FOR DEPLOYMENT</p>}</article>}</section>;
}
