import { randomBytes, scrypt, timingSafeEqual } from "node:crypto";
import { promisify } from "node:util";
import { z } from "zod";
import { database } from "@/lib/db";

const derivePassword = promisify(scrypt);
const derivedKeyLength = 64;

const email = z.string().trim().email().max(320).transform((value) => value.toLowerCase());
const password = z.string().min(12, "Use at least 12 characters.").max(128, "Use no more than 128 characters.");

export const localSignInSchema = z.object({ email, password });
export const localRegistrationSchema = localSignInSchema.extend({ displayName: z.string().trim().min(1).max(80).optional().transform((value) => value || undefined) });

type LocalAccount = { id: string; email: string; displayName: string | null };

async function derive(passwordValue: string, salt: Buffer): Promise<Buffer> {
  return await derivePassword(passwordValue, salt, derivedKeyLength) as Buffer;
}

export async function hashLocalPassword(passwordValue: string): Promise<string> {
  const salt = randomBytes(16);
  const derivedKey = await derive(passwordValue, salt);
  return `scrypt$${salt.toString("base64url")}$${derivedKey.toString("base64url")}`;
}

export async function verifyLocalPassword(passwordValue: string, storedValue: string): Promise<boolean> {
  const [algorithm, saltValue, expectedValue, ...rest] = storedValue.split("$");
  if (algorithm !== "scrypt" || !saltValue || !expectedValue || rest.length !== 0) return false;
  try {
    const salt = Buffer.from(saltValue, "base64url");
    const expected = Buffer.from(expectedValue, "base64url");
    if (salt.length !== 16 || expected.length !== derivedKeyLength) return false;
    return timingSafeEqual(await derive(passwordValue, salt), expected);
  } catch {
    return false;
  }
}

export async function createLocalAccount(input: z.infer<typeof localRegistrationSchema>): Promise<{ account?: LocalAccount; conflict: boolean }> {
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const passwordHash = await hashLocalPassword(input.password);
    const result = await client.query<LocalAccount>("INSERT INTO users(email,display_name,password_hash) VALUES($1,$2,$3) ON CONFLICT(email) DO NOTHING RETURNING id,email,display_name AS \"displayName\"", [input.email, input.displayName ?? null, passwordHash]);
    if (result.rowCount !== 1) {
      await client.query("ROLLBACK");
      return { conflict: true };
    }
    await client.query("INSERT INTO audit_events(actor_user_id,action,target_type,target_id) VALUES($1,'account.local.register','user',$2)", [result.rows[0].id, result.rows[0].id]);
    await client.query("COMMIT");
    return { account: result.rows[0], conflict: false };
  } catch (error) {
    console.error("FTEP local account registration database error", error);
    await client.query("ROLLBACK").catch(() => undefined);
    throw new Error("Local account registration failed.");
  } finally {
    client.release();
  }
}

export async function authenticateLocalAccount(input: z.infer<typeof localSignInSchema>): Promise<LocalAccount | null> {
  const result = await database().query<LocalAccount & { passwordHash: string }>("SELECT id,email,display_name AS \"displayName\",password_hash AS \"passwordHash\" FROM users WHERE email=$1 AND password_hash IS NOT NULL", [input.email]);
  const account = result.rows[0];
  if (!account || !(await verifyLocalPassword(input.password, account.passwordHash))) return null;
  return { id: account.id, email: account.email, displayName: account.displayName };
}

export function safeLocalReturnTo(value: unknown): string {
  if (typeof value !== "string" || !value.startsWith("/") || value.startsWith("//") || value.includes("\\")) return "/dashboard";
  return value;
}
