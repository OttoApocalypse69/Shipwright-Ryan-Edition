import { auth } from "@/auth";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { safeLocalReturnTo } from "@/lib/local-credentials";
import { LocalAccountForm } from "@/app/local-account/local-account-form";
import { redirect } from "next/navigation";

export const metadata = { title: "Create local account" };

export default async function SignUpPage({ searchParams }: { searchParams: Promise<{ returnTo?: string | string[] }> }) {
  const returnTo = safeLocalReturnTo((await searchParams).returnTo);
  if (await auth()) redirect(returnTo);
  return <section className="shell section"><span className="eyebrow">Local development</span><h2>Create an FTEP account</h2><article className="panel">{localCredentialsEnabled() ? <><p className="muted">This creates a development-only local FTEP account and signs you in immediately.</p><LocalAccountForm mode="sign-up" returnTo={returnTo} /></> : <p className="muted">Local account registration is disabled. Configure OAuth/OIDC for deployed FTEP.</p>}</article></section>;
}
