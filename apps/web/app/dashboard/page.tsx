import { auth, signIn, signOut } from "@/auth";
import type { Session } from "next-auth";
import Link from "next/link";
import { localCredentialsEnabled } from "@/lib/auth-config";
import { withAccountClient } from "@/lib/control-plane";

export const metadata = { title: "Dashboard" };

async function dashboardSnapshot(session: Session) {
  try {
    return await withAccountClient(session.user, async (client, account) => {
      const [entitlements, devices, compliance, sessions, achievements] = await Promise.all([
        client.query(`SELECT entitlement_key AS "key", state, granted_at AS "grantedAt" FROM entitlements WHERE user_id = $1 ORDER BY entitlement_key`, [account.id]),
        client.query(`SELECT id AS "deviceId", label, revoked_at AS "revokedAt" FROM devices WHERE user_id = $1 ORDER BY registered_at DESC`, [account.id]),
        client.query(`SELECT state, recorded_at AS "recordedAt" FROM compliance_events WHERE user_id = $1 ORDER BY recorded_at DESC LIMIT 1`, [account.id]),
        client.query(`SELECT COUNT(*)::int AS count, COALESCE(SUM(duration_ms), 0)::bigint AS "durationMs" FROM game_sessions WHERE user_id = $1 AND state = 'ENDED'`, [account.id]),
        client.query(`SELECT COUNT(*)::int AS count FROM achievement_unlocks WHERE user_id = $1`, [account.id]),
      ]);
      return { entitlements: entitlements.rows, devices: devices.rows, compliance: compliance.rows[0] ?? null, sessions: sessions.rows[0], achievements: achievements.rows[0] };
    });
  } catch {
    return null;
  }
}

export default async function DashboardPage() {
  const session = await auth();
  const githubReady = Boolean(process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET);
  const localAccounts = localCredentialsEnabled();
  if (!session?.user) {
    return <section className="shell section"><span className="eyebrow">FTEP account</span><h2>Dashboard</h2><article className="panel"><h3>Browser-based sign in</h3><p className="muted">SRE opens this flow in your browser. It never asks you to paste an access token.</p><div className="actions">{localAccounts && <><Link className="button" href="/sign-in">Sign in to local FTEP</Link><Link className="button secondary" href="/sign-up">Create local account</Link></>}{githubReady && <form action={async () => { "use server"; await signIn("github", { redirectTo: "/dashboard" }); }}><button className="button" type="submit">Continue with GitHub</button></form>}</div>{!localAccounts && !githubReady && <p className="badge">OAUTH CREDENTIALS REQUIRED FOR DEPLOYMENT</p>}</article></section>;
  }
  const snapshot = await dashboardSnapshot(session);
  const activeEntitlement = snapshot?.entitlements.find((item: { key: string; state: string }) => item.key === "ftep.library.nintendo");
  return <section className="shell section"><span className="eyebrow">FTEP account</span><h2>Dashboard</h2><article className="panel"><h3>{session.user.name ?? session.user.email}</h3><p className="muted">Role: {session.user.role}. Your account, devices, treaty, leases, sessions, and achievement state are managed by the FTEP control plane.</p><div className="metrics"><div className="metric"><span>Treaty</span><strong>{snapshot?.compliance?.state ?? "PENDING ACCEPTANCE"}</strong></div><div className="metric"><span>Zelda entitlement</span><strong>{activeEntitlement?.state ?? "NOT ISSUED"}</strong></div><div className="metric"><span>Devices</span><strong>{snapshot?.devices.length ?? "—"}</strong></div><div className="metric"><span>Achievements</span><strong>{snapshot?.achievements?.count ?? "—"}</strong></div><div className="metric"><span>Recorded sessions</span><strong>{snapshot?.sessions?.count ?? "—"}</strong></div></div><div className="actions"><Link className="button secondary" href="/devices">Devices</Link><Link className="button secondary" href="/sessions">Sessions</Link><Link className="button secondary" href="/achievements">Achievements</Link><Link className="button secondary" href="/treaty">Treaty</Link><form action={async () => { "use server"; await signOut({ redirectTo: "/" }); }}><button className="button" type="submit">Sign out</button></form></div></article></section>;
}
