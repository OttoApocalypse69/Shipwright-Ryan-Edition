import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: "SRE — Super Runtime Environment", template: "%s | SRE" },
  description: "One library. Multiple runtimes. Far too much infrastructure. Powered by FTEP.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>
    <header className="shell nav">
      <Link className="brand" href="/">SRE</Link>
      <nav className="nav-links" aria-label="Primary"><Link href="/games">Games</Link><Link href="/compatibility">Compatibility</Link><Link href="/achievements">Achievements</Link><Link href="/treaty">Accord</Link><Link href="/download">Download</Link><Link href="/dashboard">Dashboard</Link></nav>
    </header>
    <main>{children}</main>
    <footer className="shell footer">Super Runtime Environment · Powered by the FICSIT Treaty Enforcement Platform · No game data is distributed.</footer>
  </body></html>;
}
