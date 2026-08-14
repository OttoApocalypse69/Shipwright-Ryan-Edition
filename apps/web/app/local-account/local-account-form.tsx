"use client";

import Link from "next/link";
import { useActionState } from "react";
import { signInWithLocalAccount, signUpWithLocalAccount, type LocalAccountFormState } from "./actions";

const initialLocalAccountFormState: LocalAccountFormState = {};

export function LocalAccountForm({ mode, returnTo }: { mode: "sign-in" | "sign-up"; returnTo: string }) {
  const signUp = mode === "sign-up";
  const [state, action, pending] = useActionState(signUp ? signUpWithLocalAccount : signInWithLocalAccount, initialLocalAccountFormState);
  const alternate = `${signUp ? "/sign-in" : "/sign-up"}?returnTo=${encodeURIComponent(returnTo)}`;
  return <form className="account-form" action={action}>
    <input name="returnTo" type="hidden" value={returnTo} />
    {signUp && <label htmlFor="displayName">Display name<input id="displayName" name="displayName" type="text" autoComplete="name" maxLength={80} /></label>}
    <label htmlFor="email">Email<input id="email" name="email" type="email" autoComplete="email" required maxLength={320} /></label>
    <label htmlFor="password">Password<input id="password" name="password" type="password" autoComplete={signUp ? "new-password" : "current-password"} required minLength={12} maxLength={128} /></label>
    {signUp && <p className="form-note">Use at least 12 characters. This local development account stays in your local FTEP database.</p>}
    {state.error && <p aria-live="polite" className="error-text">{state.error}</p>}
    <button className="button" type="submit" disabled={pending}>{pending ? "Working…" : signUp ? "Create local account" : "Sign in"}</button>
    <p className="form-note">{signUp ? "Already have an account?" : "Need a local account?"} <Link href={alternate}>{signUp ? "Sign in" : "Create one"}</Link></p>
  </form>;
}
