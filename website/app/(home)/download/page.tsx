import type { Metadata } from 'next';
import Link from 'next/link';
import {
  ArrowUpRight,
  BookOpen,
  Download,
  Monitor,
  ShieldCheck,
  Terminal,
} from 'lucide-react';
import { DownloadCatalog } from '@/components/download-catalog';
import {
  LATEST_RELEASE_URL,
  RELEASES_URL,
  fetchLatestRelease,
} from '@/lib/releases';

export const metadata: Metadata = {
  title: 'Downloads',
  description:
    'Download Azo Type alpha builds for Linux, macOS (Sonoma 14+), and Windows, or build the native Rust editor for Typst directly from source.',
};

export default async function DownloadPage() {
  const latestRelease = await fetchLatestRelease({
    token: process.env.GITHUB_TOKEN,
  });

  return (
    <main className="mx-auto flex w-full min-w-0 max-w-5xl flex-1 flex-col px-6 py-14 md:py-20">
      {/* Header */}
      <div className="max-w-3xl">
        <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-fd-border bg-fd-card px-3 py-1 text-xs font-medium text-fd-muted-foreground">
          <Monitor className="size-3.5" aria-hidden="true" />
          <span>Alpha Desktop Releases · Cross-Platform</span>
        </div>

        <h1 className="text-3xl font-bold tracking-tight text-fd-foreground sm:text-4xl">
          Download Azo Type
        </h1>
        <p className="mt-3 text-base leading-relaxed text-fd-muted-foreground">
          Pre-built desktop alpha binaries are published on GitHub Releases for{' '}
          <strong className="font-medium text-fd-foreground">Linux</strong>{' '}
          (x86_64 &amp; aarch64),{' '}
          <strong className="font-medium text-fd-foreground">macOS</strong>{' '}
          (Apple Silicon &amp; Intel, macOS 14 Sonoma+), and{' '}
          <strong className="font-medium text-fd-foreground">Windows</strong>{' '}
          (x86_64 &amp; aarch64).
        </p>

        <div className="mt-6 flex flex-wrap items-center gap-3">
          <a
            href={latestRelease?.html_url ?? LATEST_RELEASE_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex min-h-11 items-center gap-2 rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
          >
            <Download className="size-4" aria-hidden="true" />
            <span>Latest Release on GitHub</span>
            <ArrowUpRight className="size-4" aria-hidden="true" />
          </a>
          <a
            href={RELEASES_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex min-h-11 items-center gap-2 rounded-lg border border-fd-border bg-fd-card px-4 py-2.5 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
          >
            <span>All Releases</span>
            <ArrowUpRight className="size-4" aria-hidden="true" />
          </a>
          <Link
            href="/docs/installation"
            className="inline-flex min-h-11 items-center gap-2 rounded-lg border border-fd-border px-4 py-2.5 text-sm font-medium text-fd-muted-foreground transition-colors hover:bg-fd-accent hover:text-fd-foreground"
          >
            <BookOpen className="size-4" aria-hidden="true" />
            <span>Installation Docs</span>
          </Link>
        </div>
      </div>

      <DownloadCatalog release={latestRelease} />

      {/* Post-Install & Verification + Source Build */}
      <section className="mt-12 grid w-full min-w-0 grid-cols-1 gap-6 md:grid-cols-2">
        <div className="min-w-0 overflow-hidden rounded-xl border border-fd-border bg-fd-card p-6">
          <div className="flex items-center gap-2 text-sm font-semibold text-fd-foreground">
            <ShieldCheck className="size-4" aria-hidden="true" />
            <span>Post-Install &amp; Verification Notes</span>
          </div>
          <ul className="mt-3 space-y-2.5 break-words text-sm leading-relaxed text-fd-muted-foreground">
            <li>
              <strong className="text-fd-foreground">
                Built-in Typst Compiler:
              </strong>{' '}
              <strong>Azo Type</strong> bundles Typst 0.15.1
              and 17 embedded fonts out of the box—no separate{' '}
              <code className="font-mono">typst</code> CLI installation is
              required.
            </li>
            <li>
              <strong className="text-fd-foreground">
                macOS Sonoma 14+ &amp; Signing:
              </strong>{' '}
              Both Apple Silicon and Intel macOS builds target macOS 14 Sonoma
              or newer and are ad-hoc signed. If Gatekeeper blocks first launch,
              Control-click{' '}
              <code className="font-mono">Azo Type.app</code> and choose{' '}
              <strong className="text-fd-foreground">Open</strong>.
            </li>
            <li>
              <strong className="text-fd-foreground">
                Optional Downloads:
              </strong>{' '}
              Legacy Typst versions (<code className="font-mono">0.14.2</code>,{' '}
              <code className="font-mono">0.13.1</code>,{' '}
              <code className="font-mono">0.12.0</code>) and Tier-1 offline
              spell/grammar packs are verified with SHA-256 checksums before
              activation.
            </li>
            <li>
              <strong className="text-fd-foreground">
                Smoke Verification:
              </strong>{' '}
              Run{' '}
              <code className="font-mono">
                azo-type --screenshot /tmp/out.png
              </code>{' '}
              to verify graphics initialization and headless rendering.
            </li>
          </ul>
        </div>

        <div className="min-w-0 overflow-hidden rounded-xl border border-fd-border bg-fd-card p-6">
          <div className="flex items-center gap-2 text-sm font-semibold text-fd-foreground">
            <Terminal className="size-4" aria-hidden="true" />
            <span>Build Optimized Binary from Source</span>
          </div>
          <p className="mt-2 text-sm text-fd-muted-foreground">
            Requires Rust 1.80+ and (on Linux){' '}
            <code className="font-mono">libfontconfig1-dev</code> plus
            X11/Wayland headers.
          </p>
          <pre className="mt-3 max-w-full overflow-x-auto rounded-lg border border-fd-border bg-fd-secondary/50 p-3.5 font-mono text-xs leading-6 text-fd-foreground">
            <code>{`git clone https://github.com/azocloud/azo-type.git
cd azo-type
cargo build --release --manifest-path app/Cargo.toml
./target/release/azo-type`}</code>
          </pre>
        </div>
      </section>
    </main>
  );
}
