import { Pool } from "pg";

let pool: Pool | undefined;
export function database(): Pool {
  if (!process.env.DATABASE_URL) throw new Error("DATABASE_URL is not configured");
  pool ??= new Pool({ connectionString: process.env.DATABASE_URL, ssl: process.env.NODE_ENV === "production" ? { rejectUnauthorized: true } : false, max: 5, connectionTimeoutMillis: 3_000 });
  return pool;
}

export async function query<T extends Record<string, unknown>>(text: string, values: readonly unknown[] = []): Promise<T[]> {
  const result = await database().query<T>(text, [...values]);
  return result.rows;
}
