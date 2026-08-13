import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vitest";

test("PostgreSQL migration covers every required domain table", () => {
  const sql = readFileSync(resolve(process.cwd(), "../../database/migrations/0001_ftep_core.sql"), "utf8").toLowerCase();
  const tables = ["users","oauth_accounts","devices","games","game_variants","game_installations","runtimes","runtime_compatibility","game_sessions","achievements","achievement_unlocks","achievement_progress","treaties","treaty_versions","treaty_parties","treaty_articles","obligations","ratifications","compliance_events","incidents","remediations","entitlements","entitlement_leases","revocations","releases","scope_requests","audit_events"];
  for (const table of tables) expect(sql).toContain(`create table ${table}`);
  expect(sql).not.toMatch(/game_binary|save_contents|rom_blob/);
});

test("required public and admin routes exist", () => {
  const routes = ["download","library","games","compatibility","achievements","treaty","dashboard","devices","sessions","status","privacy","security","connect","admin"];
  for (const route of routes) expect(existsSync(resolve(process.cwd(), `app/${route}/page.tsx`))).toBe(true);
  expect(existsSync(resolve(process.cwd(), "app/admin/[section]/page.tsx"))).toBe(true);
  expect(existsSync(resolve(process.cwd(), "app/api/ratifications/route.ts"))).toBe(true);
});
