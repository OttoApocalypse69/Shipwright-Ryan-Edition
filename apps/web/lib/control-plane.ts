import type { PoolClient } from "pg";
import { database } from "@/lib/db";
import { roleForEmail, type FtepRole } from "@/lib/auth-config";

export type ControlPlaneAccount = {
  id: string;
  email: string;
  displayName: string | null;
  role: FtepRole;
};

export type SessionIdentity = {
  email?: string | null;
  name?: string | null;
};

export function requireIdentity(identity: SessionIdentity): { email: string; displayName: string | null } {
  const email = identity.email?.trim().toLowerCase();
  if (!email) throw new Error("FTEP_AUTH_REQUIRED");
  return { email, displayName: identity.name?.trim() || null };
}

export async function upsertAccount(
  client: PoolClient,
  identity: SessionIdentity,
): Promise<ControlPlaneAccount> {
  const normalized = requireIdentity(identity);
  const configuredRole = roleForEmail(normalized.email);
  const result = await client.query<ControlPlaneAccount>(
    `INSERT INTO users(email, display_name, role, last_authenticated_at)
     VALUES ($1, $2, $3, now())
     ON CONFLICT(email) DO UPDATE SET
       display_name = COALESCE(EXCLUDED.display_name, users.display_name),
       role = CASE WHEN users.role = 'admin' OR EXCLUDED.role = 'admin' THEN 'admin' ELSE 'user' END,
       last_authenticated_at = now(),
       updated_at = now()
     RETURNING id, email, display_name AS "displayName", role`,
    [normalized.email, normalized.displayName, configuredRole],
  );
  const account = result.rows[0];
  if (!account) throw new Error("FTEP_ACCOUNT_UNAVAILABLE");
  return account;
}

export async function findAccountByEmail(client: PoolClient, email: string): Promise<ControlPlaneAccount | null> {
  const result = await client.query<ControlPlaneAccount>(
    `SELECT id, email, display_name AS "displayName", role
       FROM users
      WHERE email = $1`,
    [email.trim().toLowerCase()],
  );
  return result.rows[0] ?? null;
}

export async function readAccountByEmail(email: string): Promise<ControlPlaneAccount | null> {
  const client = await database().connect();
  try {
    return await findAccountByEmail(client, email);
  } finally {
    client.release();
  }
}

export async function persistOAuthIdentity(input: {
  email: string;
  displayName?: string | null;
  provider: string;
  providerAccountId: string;
}): Promise<ControlPlaneAccount> {
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const account = await upsertAccount(client, { email: input.email, name: input.displayName });
    await client.query(
      `INSERT INTO oauth_accounts(user_id, provider, provider_account_id)
       VALUES ($1, $2, $3)
       ON CONFLICT(provider, provider_account_id) DO UPDATE SET user_id = EXCLUDED.user_id`,
      [account.id, input.provider, input.providerAccountId],
    );
    await recordAudit(client, account.id, "auth.identity.linked", "user", account.id, {
      provider: input.provider,
    });
    await client.query("COMMIT");
    return account;
  } catch (error) {
    await client.query("ROLLBACK").catch(() => undefined);
    throw error;
  } finally {
    client.release();
  }
}

export async function withAccountTransaction<T>(
  identity: SessionIdentity,
  operation: (client: PoolClient, account: ControlPlaneAccount) => Promise<T>,
): Promise<T> {
  const client = await database().connect();
  try {
    await client.query("BEGIN");
    const account = await upsertAccount(client, identity);
    const result = await operation(client, account);
    await client.query("COMMIT");
    return result;
  } catch (error) {
    await client.query("ROLLBACK").catch(() => undefined);
    throw error;
  } finally {
    client.release();
  }
}

export async function withAccountClient<T>(
  identity: SessionIdentity,
  operation: (client: PoolClient, account: ControlPlaneAccount) => Promise<T>,
): Promise<T> {
  const client = await database().connect();
  try {
    const account = await upsertAccount(client, identity);
    return await operation(client, account);
  } finally {
    client.release();
  }
}

export async function recordAudit(
  client: PoolClient,
  actorUserId: string | null,
  action: string,
  targetType: string,
  targetId: string | null,
  metadata: Record<string, unknown> = {},
  requestIdValue?: string,
): Promise<void> {
  await client.query(
    `INSERT INTO audit_events(actor_user_id, action, target_type, target_id, request_id, metadata)
     VALUES ($1, $2, $3, $4, $5, $6::jsonb)`,
    [actorUserId, action, targetType, targetId, requestIdValue ?? null, JSON.stringify(metadata)],
  );
}

export function isUniqueViolation(error: unknown): boolean {
  return Boolean(error && typeof error === "object" && "code" in error && (error as { code?: unknown }).code === "23505");
}

export function isForeignKeyViolation(error: unknown): boolean {
  return Boolean(error && typeof error === "object" && "code" in error && (error as { code?: unknown }).code === "23503");
}

export function requireAdminAccount(account: ControlPlaneAccount): void {
  if (account.role !== "admin") throw new Error("FTEP_ADMIN_REQUIRED");
}
