import { auth } from "@/auth";
import Link from "next/link";
import { redirect } from "next/navigation";

const sections = ["users", "games", "runtimes", "compatibility", "treaties", "compliance", "releases", "audit"];
export default async function AdminLayout({ children }: { children: React.ReactNode }) {
  const session = await auth();
  if (!session?.user) redirect("/dashboard");
  if (session.user.role !== "admin") redirect("/dashboard?error=admin-required");
  return <section className="shell section"><span className="eyebrow">FTEP control plane</span><h2>Administration</h2><nav className="admin-nav"><Link href="/admin">Overview</Link>{sections.map((section) => <Link key={section} href={`/admin/${section}`}>{section}</Link>)}</nav>{children}</section>;
}
