import { auth } from "@/auth";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { safeLocalReturnTo } from "@/lib/local-credentials";
import { LocalAccountForm } from "@/app/local-account/local-account-form";
import { redirect } from "next/navigation";

export const metadata = { title: "Sign in" };

export default async function SignInPage({ searchParams }: { searchParams: Promise<{ returnTo?: string | string[] }> }) {
  const returnTo = safeLocalReturnTo((await searchParams).returnTo);
  if (await auth()) redirect(returnTo);
  return <section className="shell section"><span className="eyebrow">Local development</span><h2>Sign in to FTEP</h2><article className="panel">{localCredentialsEnabled() ? <><p className="muted">Use the local account you created for this FTEP development environment.</p><LocalAccountForm mode="sign-in" returnTo={returnTo} /></> : <p className="muted">Local account sign-in is disabled. Configure OAuth/OIDC for deployed FTEP.</p>}</article></section>;
}
