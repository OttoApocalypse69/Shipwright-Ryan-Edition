export type FtepRole = "user" | "admin";

export function localCredentialsEnabled(env: Readonly<Record<string, string | undefined>> = process.env): boolean {
  return env.FTEP_LOCAL_CREDENTIALS_ENABLED === "true" && env.NODE_ENV !== "production";
}

export function roleForEmail(email: string | null | undefined, configured = process.env.FTEP_ADMIN_EMAILS): FtepRole {
  const administrators = new Set((configured ?? "").split(",").map((value) => value.trim().toLowerCase()).filter(Boolean));
  return email && administrators.has(email.toLowerCase()) ? "admin" : "user";
}

export function configuredProviderIds(env: Readonly<Record<string, string | undefined>> = process.env): string[] {
  const ids: string[] = [];
  if (env.AUTH_GITHUB_ID && env.AUTH_GITHUB_SECRET) ids.push("github");
  if (env.AUTH_GOOGLE_ID && env.AUTH_GOOGLE_SECRET) ids.push("google");
  if (env.AUTH_DISCORD_ID && env.AUTH_DISCORD_SECRET) ids.push("discord");
  if (env.AUTH_OIDC_ID && env.AUTH_OIDC_SECRET && env.AUTH_OIDC_ISSUER) ids.push("oidc");
  return ids;
}
