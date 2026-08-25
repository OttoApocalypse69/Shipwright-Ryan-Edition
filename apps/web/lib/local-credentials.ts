import { randomBytes, scrypt, timingSafeEqual } from "node:crypto";
import { mkdir, readFile, rename, unlink, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { promisify } from "node:util";
import type { PoolClient } from "pg";
import { z } from "zod";
import { database } from "@/lib/db";

const derivePassword = promisify(scrypt);
const derivedKeyLength = 64;

const email = z.string().trim().email().max(320).transform((value) => value.toLowerCase());
const password = z.string().min(12, "Use at least 12 characters.").max(128, "Use no more than 128 characters.");

export const localSignInSchema = z.object({ email, password });
export const localRegistrationSchema = localSignInSchema.extend({ displayName: z.string().trim().min(1).max(80).optional().transform((value) => value || undefined) });

type LocalAccount = { id: string; email: string; displayName: string | null };
type StoredLocalAccount = LocalAccount & { passwordHash: string };

function localAccountStorePath(): string {
  return process.env.FTEP_LOCAL_ACCOUNT_STORE
    ?? join(process.env.LOCALAPPDATA ?? homedir(), "Shipwright Ryan Edition", "ftep", "local-accounts.json");
}

async function readLocalAccountStore(): Promise<StoredLocalAccount[]> {
  try {
    const parsed: unknown = JSON.parse(await readFile(/* turbopackIgnore: true */ localAccountStorePath(), "utf8"));
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is StoredLocalAccount => {
      if (!entry || typeof entry !== "object") return false;
      const candidate = entry as Partial<StoredLocalAccount>;
      return typeof candidate.id === "string"
        && typeof candidate.email === "string"
        && (typeof candidate.displayName === "string" || candidate.displayName === null)
        && typeof candidate.passwordHash === "string";
    });
  } catch (error: unknown) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    console.error("FTEP local account store could not be read", error);
    return [];
  }
}

async function writeLocalAccountStore(accounts: StoredLocalAccount[]): Promise<void> {
  const destination = localAccountStorePath();
  const temporary = `${destination}.${randomBytes(8).toString("hex")}.tmp`;
  await mkdir(dirname(destination), { recursive: true });
  try {
    await writeFile(temporary, `${JSON.stringify(accounts, null, 2)}\n`, { encoding: "utf8", mode: 0o600 });
    await rename(temporary, destination);
  } finally {
    await unlink(temporary).catch(() => undefined);
  }
}

export function isLocalDatabaseUnavailable(error: unknown): boolean {
  const code = (error as { code?: unknown } | undefined)?.code;
  // A local PostgreSQL container can outlive the checkout that created it.
  // Treat missing control-plane migrations like an unavailable local store so
  // development can use the durable file-backed FTEP state until migrations
  // are applied. Production never enables this fallback.
  if (typeof code === "string" && ["ECONNREFUSED", "ENOTFOUND", "ETIMEDOUT", "ECONNRESET", "EPIPE", "08001", "3D000", "42P01", "42703", "42704", "57P03", "53300"].includes(code)) return true;
  return error instanceof Error && (error.message === "DATABASE_URL is not configured" || /connect ECONNREFUSED|connect ETIMEDOUT|connection terminated unexpectedly/i.test(error.message));
}

let accountStoreOperation = Promise.resolve();

async function withAccountStoreLock<T>(operation: () => Promise<T>): Promise<T> {
  const previous = accountStoreOperation;
  let release!: () => void;
  accountStoreOperation = new Promise<void>((resolve) => { release = resolve; });
  await previous;
  try {
    return await operation();
  } finally {
    release();
  }
}

async function createFileBackedLocalAccount(input: z.infer<typeof localRegistrationSchema>): Promise<{ account?: LocalAccount; conflict: boolean }> {
  return await withAccountStoreLock(async () => {
    const accounts = await readLocalAccountStore();
    if (accounts.some((account) => account.email === input.email)) return { conflict: true };
    const account: StoredLocalAccount = {
      id: crypto.randomUUID(),
      email: input.email,
      displayName: input.displayName ?? null,
      passwordHash: await hashLocalPassword(input.password),
    };
    await writeLocalAccountStore([...accounts, account]);
    return { account: { id: account.id, email: account.email, displayName: account.displayName }, conflict: false };
  });
}

async function authenticateFileBackedLocalAccount(input: z.infer<typeof localSignInSchema>): Promise<LocalAccount | null> {
  const account = (await readLocalAccountStore()).find((candidate) => candidate.email === input.email);
  if (!account || !(await verifyLocalPassword(input.password, account.passwordHash))) return null;
  return { id: account.id, email: account.email, displayName: account.displayName };
}

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
  // The file store is the durable source for development accounts. This also
  // prevents a PostgreSQL restart from creating a second account with the same
  // email after the first account was created while the database was offline.
  if (await localAccountByEmail(input.email)) return { conflict: true };
  let client: PoolClient | undefined;
  try {
    const connectedClient = await database().connect();
    client = connectedClient;
    await connectedClient.query("BEGIN");
    const passwordHash = await hashLocalPassword(input.password);
    const result = await connectedClient.query<LocalAccount>("INSERT INTO users(email,display_name,password_hash) VALUES($1,$2,$3) ON CONFLICT(email) DO NOTHING RETURNING id,email,display_name AS \"displayName\"", [input.email, input.displayName ?? null, passwordHash]);
    if (result.rowCount !== 1) {
      await connectedClient.query("ROLLBACK");
      return { conflict: true };
    }
    await connectedClient.query("INSERT INTO audit_events(actor_user_id,action,target_type,target_id) VALUES($1,'account.local.register','user',$2)", [result.rows[0].id, result.rows[0].id]);
    await connectedClient.query("COMMIT");
    return { account: result.rows[0], conflict: false };
  } catch (error) {
    if (!isLocalDatabaseUnavailable(error)) console.error("FTEP local account registration database error", error);
    await client?.query("ROLLBACK").catch(() => undefined);
    if (isLocalDatabaseUnavailable(error)) return await createFileBackedLocalAccount(input);
    throw new Error("Local account registration failed.");
  } finally {
    client?.release();
  }
}

export async function authenticateLocalAccount(input: z.infer<typeof localSignInSchema>): Promise<LocalAccount | null> {
  try {
    const result = await database().query<LocalAccount & { passwordHash: string }>("SELECT id,email,display_name AS \"displayName\",password_hash AS \"passwordHash\" FROM users WHERE email=$1 AND password_hash IS NOT NULL", [input.email]);
    const account = result.rows[0];
    if (account) return (await verifyLocalPassword(input.password, account.passwordHash)) ? { id: account.id, email: account.email, displayName: account.displayName } : null;
    // A local account may have been created during a database outage. Keep
    // sign-in working when PostgreSQL comes back with an empty/new database.
    return await authenticateFileBackedLocalAccount(input);
  } catch (error) {
    if (!isLocalDatabaseUnavailable(error)) throw error;
    console.warn("FTEP local account database is unavailable; using the local account store.");
  }
  return await authenticateFileBackedLocalAccount(input);
}

export type LocalAccountRecord = LocalAccount;

export async function localAccountByEmail(value: string): Promise<LocalAccountRecord | null> {
  const normalizedEmail = value.trim().toLowerCase();
  const account = (await readLocalAccountStore()).find((candidate) => candidate.email === normalizedEmail);
  return account ? { id: account.id, email: account.email, displayName: account.displayName } : null;
}

export function safeLocalReturnTo(value: unknown): string {
  if (typeof value !== "string" || !value.startsWith("/") || value.startsWith("//") || value.includes("\\")) return "/dashboard";
  return value;
}
