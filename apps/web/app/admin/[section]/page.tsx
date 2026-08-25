import { notFound } from "next/navigation";
import { AdminDataPanel, type Section } from "../admin-data-panel";
const descriptions: Record<string, string> = {
  users: "Account identities, roles, OAuth links, and device registrations.", games: "Catalog visibility and library policy metadata.", runtimes: "Supported runtime versions and known-broken classifications.", compatibility: "Game + variant + runtime + version-range compatibility decisions.", treaties: "Versioned Accord articles, parties, obligations, and ratifications.", compliance: "Warnings, material breaches, remediation, restoration, and scope requests.", releases: "Stable, beta, and nightly signed release metadata.", audit: "Append-only security and administrative action history.",
};
type Props = { params: Promise<{ section: string }> };
export function generateStaticParams() { return Object.keys(descriptions).map((section) => ({ section })); }
export default async function AdminSectionPage({ params }: Props) { const { section } = await params; const description = descriptions[section]; if (!description) notFound(); return <><article className="panel"><span className="eyebrow">Admin module</span><h3>{section}</h3><p className="lede">{description}</p><p className="muted">Production writes require an authenticated admin role, CSRF-protected browser request, PostgreSQL transaction, and audit event.</p></article><AdminDataPanel section={section as Section} /></>; }
