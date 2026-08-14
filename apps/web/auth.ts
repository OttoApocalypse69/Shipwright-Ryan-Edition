import NextAuth from "next-auth";
import Discord from "next-auth/providers/discord";
import GitHub from "next-auth/providers/github";
import Google from "next-auth/providers/google";
import Credentials from "next-auth/providers/credentials";
import type { Provider } from "next-auth/providers";
import { localCredentialsEnabled, roleForEmail } from "@/lib/auth-config";
import type { FtepRole } from "@/lib/auth-config";
import { authenticateLocalAccount, localSignInSchema } from "@/lib/local-credentials";
export type { FtepRole } from "@/lib/auth-config";

function providers(): Provider[] {
  const configured: Provider[] = [];
  if (localCredentialsEnabled()) {
    configured.push(Credentials({
      id: "local",
      name: "FTEP account",
      credentials: {
        email: { label: "Email", type: "email" },
        password: { label: "Password", type: "password" },
      },
      async authorize(credentials) {
        const parsed = localSignInSchema.safeParse(credentials);
        if (!parsed.success) return null;
        const account = await authenticateLocalAccount(parsed.data);
        return account && { id: account.id, email: account.email, name: account.displayName ?? account.email };
      },
    }));
  }
  if (process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET) configured.push(GitHub({ clientId: process.env.AUTH_GITHUB_ID, clientSecret: process.env.AUTH_GITHUB_SECRET }));
  if (process.env.AUTH_GOOGLE_ID && process.env.AUTH_GOOGLE_SECRET) configured.push(Google({ clientId: process.env.AUTH_GOOGLE_ID, clientSecret: process.env.AUTH_GOOGLE_SECRET }));
  if (process.env.AUTH_DISCORD_ID && process.env.AUTH_DISCORD_SECRET) configured.push(Discord({ clientId: process.env.AUTH_DISCORD_ID, clientSecret: process.env.AUTH_DISCORD_SECRET }));
  if (process.env.AUTH_OIDC_ID && process.env.AUTH_OIDC_SECRET && process.env.AUTH_OIDC_ISSUER) {
    configured.push({
      id: "oidc",
      name: "Organisation OIDC",
      type: "oidc",
      issuer: process.env.AUTH_OIDC_ISSUER,
      clientId: process.env.AUTH_OIDC_ID,
      clientSecret: process.env.AUTH_OIDC_SECRET,
    });
  }
  return configured;
}

export const { handlers, auth, signIn, signOut } = NextAuth({
  providers: providers(),
  session: { strategy: "jwt", maxAge: 8 * 60 * 60 },
  pages: { signIn: "/dashboard" },
  callbacks: {
    jwt({ token }) { token.role = roleForEmail(token.email); return token; },
    session({ session, token }) { session.user.role = token.role === "admin" ? "admin" : "user"; return session; },
    authorized({ auth: session, request }) {
      return request.nextUrl.pathname.startsWith("/admin") ? Boolean(session?.user) : true;
    },
  },
  cookies: {
    sessionToken: {
      name: process.env.NODE_ENV === "production" ? "__Secure-ftep.session-token" : "ftep.session-token",
      options: { httpOnly: true, sameSite: "lax", path: "/", secure: process.env.NODE_ENV === "production" },
    },
  },
});

declare module "next-auth" {
  interface Session { user: { name?: string | null; email?: string | null; image?: string | null; role: FtepRole } }
}
