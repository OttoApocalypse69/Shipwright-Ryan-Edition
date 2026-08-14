"use server";

import { AuthError } from "next-auth";
import { signIn } from "@/auth";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { createLocalAccount, localRegistrationSchema, localSignInSchema, safeLocalReturnTo } from "@/lib/local-credentials";

export type LocalAccountFormState = { error?: string };

function developmentAccountsEnabled(): boolean {
  return localCredentialsEnabled();
}

export async function signInWithLocalAccount(_: LocalAccountFormState, formData: FormData): Promise<LocalAccountFormState> {
  if (!developmentAccountsEnabled()) return { error: "Local account sign-in is unavailable in this environment." };
  const parsed = localSignInSchema.safeParse({ email: formData.get("email"), password: formData.get("password") });
  if (!parsed.success) return { error: "Enter a valid email address and password." };
  try {
    await signIn("local", { ...parsed.data, redirectTo: safeLocalReturnTo(formData.get("returnTo")) });
  } catch (error) {
    if (error instanceof AuthError) return { error: "Email or password is incorrect." };
    throw error;
  }
  return { error: "Sign-in did not complete. Please try again." };
}

export async function signUpWithLocalAccount(_: LocalAccountFormState, formData: FormData): Promise<LocalAccountFormState> {
  if (!developmentAccountsEnabled()) return { error: "Local account registration is unavailable in this environment." };
  const parsed = localRegistrationSchema.safeParse({ email: formData.get("email"), password: formData.get("password"), displayName: formData.get("displayName") });
  if (!parsed.success) return { error: parsed.error.issues[0]?.message ?? "Enter a valid email and password." };
  let result: Awaited<ReturnType<typeof createLocalAccount>>;
  try {
    result = await createLocalAccount(parsed.data);
  } catch {
    return { error: "FTEP could not create the local account. Confirm that the local database is running and try again." };
  }
  if (result.conflict) return { error: "This email already has an account. Sign in instead." };
  try {
    await signIn("local", { email: parsed.data.email, password: parsed.data.password, redirectTo: safeLocalReturnTo(formData.get("returnTo")) });
  } catch (error) {
    if (error instanceof AuthError) return { error: "Account created, but sign-in did not complete. Try signing in again." };
    throw error;
  }
  return { error: "Account creation did not complete. Please try again." };
}
