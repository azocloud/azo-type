import type { Metadata } from 'next';
import Link from 'next/link';
import {
  AlertCircle,
  ArrowUpRight,
  BookOpen,
  CheckCircle2,
  Download,
  Monitor,
  ShieldCheck,
  Terminal,
} from 'lucide-react';

export const metadata: Metadata = {
  title: 'Download',
  description:
    'Download azo-type for Linux, macOS, and Windows, or build the native Rust Typst IDE directly from source.',
};

interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface GitHubRelease {
  tag_name: string;
  name: string;
  html_url: string;
  published_at?: string;
  assets: GitHubReleaseAsset[];
}

const RELEASES_URL = 'https://github.com/azocloud/azo-type/releases';
const LATEST_RELEASE_URL = 'https://github.com/azocloud/azo-type/releases/latest';
const GITHUB_API_LATEST =
  'https://api.github.com/repos/azocloud/azo-type/releases/latest';

async function fetchLatestRelease(): Promise<GitHubRelease | null> {
  try {
    const res = await fetch(GITHUB_API_LATEST, {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': 'azo-type-website',
      },
      signal: AbortSignal.timeout(3500),
    });
    if (!res.ok) return null;
    const data = (await res.json()) as GitHubRelease;
    if (!data?.tag_name) return null;
    return data;
  } catch {
    return null;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function matchAssets(
  assets: GitHubReleaseAsset[],
  patterns: RegExp[],
): GitHubReleaseAsset[] {
  return assets.filter((asset) =>
    patterns.some((pattern) => pattern.test(asset.name)),
  );
}

interface PlatformCard {
  id: string;
  name: string;
  architectures: string;
  notes: string;
  requirements: string[];
  patterns: RegExp[];
}

const platforms: PlatformCard[] = [
  {
    id: 'linux',
    name: 'Linux',
    architectures: 'x86_64 · aarch64 (Wayland & X11)',
    notes:
      'In-window desktop menu bar, GTK/portal native file dialogs via rfd, and hardware-accelerated OpenGL (glow) viewport.',
    requirements: [
      'Wayland or X11 display server',
      'fontconfig runtime & OpenGL drivers',
      'Building from source requires libfontconfig1-dev and X11/Wayland dev headers',
    ],
    patterns: [/linux/i, /\.AppImage$/i, /\.deb$/i, /\.tar\.(gz|xz)$/i],
  },
  {
    id: 'macos',
    name: 'macOS',
    architectures: 'Apple Silicon (aarch64) · Intel (x86_64)',
    notes:
      'Integrates a native macOS screen-top menu bar via muda, Cmd-based keyboard shortcuts, and native NSOpenPanel / NSSavePanel dialogs.',
    requirements: [
      'macOS 12 Monterey or newer recommended',
      'Universal or architecture-specific .app / .dmg / tarball',
      'Xcode Command Line Tools required when building from source',
    ],
    patterns: [/macos|darwin|apple/i, /\.dmg$/i],
  },
  {
    id: 'windows',
    name: 'Windows',
    architectures: 'x86_64 · aarch64 (MSVC)',
    notes:
      'In-window menu bar, Win32 native file pickers, embedded application icon resources, and Ctrl-based shortcuts.',
    requirements: [
      'Windows 10 or Windows 11 (64-bit)',
      'Microsoft Visual C++ Redistributable / OpenGL-capable GPU driver',
      'MSVC Rust toolchain required when building from source',
    ],
    patterns: [/windows|win64|win32|msvc/i, /\.exe$/i, /\.msi$/i, /\.zip$/i],
  },
];

export default async function DownloadPage() {
  const latestRelease = await fetchLatestRelease();
  const releaseHref = latestRelease?.html_url ?? LATEST_RELEASE_URL;

  return (
    <main className="mx-auto flex w-full max-w-5xl flex-1 flex-col px-6 py-14 md:py-20">
      {/* Header */}
      <div className="max-w-3xl">
        <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-fd-border bg-fd-card px-3 py-1 text-xs font-medium text-fd-muted-foreground">
          <Monitor className="size-3.5" aria-hidden="true" />
          <span>Cross-Platform Desktop Releases</span>
        </div>

        <h1 className="text-3xl font-bold tracking-tight text-fd-foreground sm:text-4xl">
          Download azo-type
        </h1>
        <p className="mt-3 text-base leading-relaxed text-fd-muted-foreground">
          Pre-built desktop binaries and release archives are published on GitHub Releases for{' '}
          <strong className="font-medium text-fd-foreground">Linux</strong>,{' '}
          <strong className="font-medium text-fd-foreground">macOS</strong>, and{' '}
          <strong className="font-medium text-fd-foreground">Windows</strong>.
        </p>

        <div className="mt-6 flex flex-wrap items-center gap-3">
          <a
            href={LATEST_RELEASE_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
          >
            <Download className="size-4" aria-hidden="true" />
            <span>
              {latestRelease
                ? `Latest Release (${latestRelease.tag_name})`
                : 'Latest Release on GitHub'}
            </span>
            <ArrowUpRight className="size-4" aria-hidden="true" />
          </a>
          <a
            href={RELEASES_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-card px-4 py-2.5 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
          >
            <span>All Releases</span>
            <ArrowUpRight className="size-4" aria-hidden="true" />
          </a>
          <Link
            href="/docs/installation"
            className="inline-flex items-center gap-2 rounded-lg border border-fd-border px-4 py-2.5 text-sm font-medium text-fd-muted-foreground transition-colors hover:bg-fd-accent hover:text-fd-foreground"
          >
            <BookOpen className="size-4" aria-hidden="true" />
            <span>Installation Docs</span>
          </Link>
        </div>
      </div>

      {/* Release Status Banner */}
      <div className="mt-8 rounded-xl border border-fd-border bg-fd-card p-4 text-sm">
        {latestRelease ? (
          <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-2 font-medium text-fd-foreground">
              <CheckCircle2 className="size-4 text-emerald-500" aria-hidden="true" />
              <span>
                Latest published release:{' '}
                <span className="font-mono">{latestRelease.tag_name}</span>
              </span>
            </div>
            <a
              href={latestRelease.html_url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 text-xs font-medium text-fd-primary hover:underline"
            >
              <span>View release notes</span>
              <ArrowUpRight className="size-3.5" aria-hidden="true" />
            </a>
          </div>
        ) : (
          <div className="flex items-start gap-2.5 text-fd-muted-foreground">
            <AlertCircle className="mt-0.5 size-4 shrink-0 text-amber-500" aria-hidden="true" />
            <div>
              <p className="font-medium text-fd-foreground">
                Early Pre-Alpha (v0.3.0 in <code className="font-mono">app/Cargo.toml</code>)
              </p>
              <p className="mt-1">
                If no pre-built binary is published on{' '}
                <a
                  href={RELEASES_URL}
                  target="_blank"
                  rel="noreferrer"
                  className="underline underline-offset-4 hover:text-fd-foreground"
                >
                  GitHub Releases
                </a>{' '}
                yet, you can build <code className="font-mono">azo-type</code> directly from source with Rust 1.80+.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Platform Cards */}
      <section className="mt-10 grid gap-5 md:grid-cols-3">
        {platforms.map((platform) => {
          const matchedAssets = latestRelease
            ? matchAssets(latestRelease.assets, platform.patterns)
            : [];

          return (
            <div
              key={platform.id}
              className="flex flex-col justify-between rounded-xl border border-fd-border bg-fd-card p-6"
            >
              <div>
                <h2 className="text-xl font-semibold text-fd-foreground">
                  {platform.name}
                </h2>
                <p className="mt-1 font-mono text-xs text-fd-muted-foreground">
                  {platform.architectures}
                </p>
                <p className="mt-3 text-sm leading-relaxed text-fd-muted-foreground">
                  {platform.notes}
                </p>

                <div className="mt-4 border-t border-fd-border pt-4">
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-fd-muted-foreground">
                    System Requirements
                  </h3>
                  <ul className="mt-2 space-y-1.5 text-xs text-fd-muted-foreground">
                    {platform.requirements.map((req) => (
                      <li key={req} className="flex items-start gap-2">
                        <span className="mt-1 size-1 shrink-0 rounded-full bg-fd-muted-foreground" />
                        <span>{req}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              </div>

              <div className="mt-6 space-y-2">
                {matchedAssets.length > 0 ? (
                  matchedAssets.map((asset) => (
                    <a
                      key={asset.name}
                      href={asset.browser_download_url}
                      target="_blank"
                      rel="noreferrer"
                      className="flex w-full items-center justify-between rounded-lg border border-fd-border bg-fd-secondary/60 px-3.5 py-2 text-xs font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
                    >
                      <span className="truncate font-mono">{asset.name}</span>
                      <span className="ml-2 shrink-0 text-fd-muted-foreground">
                        {formatBytes(asset.size)}
                      </span>
                    </a>
                  ))
                ) : (
                  <a
                    href={releaseHref}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex w-full items-center justify-center gap-2 rounded-lg border border-fd-border bg-fd-secondary/60 px-4 py-2 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
                  >
                    <Download className="size-4" aria-hidden="true" />
                    <span>Get {platform.name} Build</span>
                    <ArrowUpRight className="size-3.5" aria-hidden="true" />
                  </a>
                )}
              </div>
            </div>
          );
        })}
      </section>

      {/* Post-Install & Verification + Source Build */}
      <section className="mt-12 grid gap-6 md:grid-cols-2">
        <div className="rounded-xl border border-fd-border bg-fd-card p-6">
          <div className="flex items-center gap-2 text-sm font-semibold text-fd-foreground">
            <ShieldCheck className="size-4" aria-hidden="true" />
            <span>Post-Install & Verification Notes</span>
          </div>
          <ul className="mt-3 space-y-2.5 text-sm leading-relaxed text-fd-muted-foreground">
            <li>
              <strong className="text-fd-foreground">Built-in Typst Compiler:</strong>{' '}
              <code className="font-mono">azo-type</code> bundles Typst 0.15.1 and 17 embedded fonts out of the box—no separate <code className="font-mono">typst</code> CLI installation is required.
            </li>
            <li>
              <strong className="text-fd-foreground">Optional Downloads:</strong>{' '}
              Legacy Typst versions (<code className="font-mono">0.14.2</code>,{' '}
              <code className="font-mono">0.13.1</code>,{' '}
              <code className="font-mono">0.12.0</code>) and Tier-1 offline spell/grammar packs are verified with SHA-256 checksums before activation.
            </li>
            <li>
              <strong className="text-fd-foreground">Smoke Verification:</strong>{' '}
              Run <code className="font-mono">azo-type --screenshot /tmp/out.png</code> to verify graphics initialization and headless rendering.
            </li>
          </ul>
        </div>

        <div className="rounded-xl border border-fd-border bg-fd-card p-6">
          <div className="flex items-center gap-2 text-sm font-semibold text-fd-foreground">
            <Terminal className="size-4" aria-hidden="true" />
            <span>Build Optimized Binary from Source</span>
          </div>
          <p className="mt-2 text-sm text-fd-muted-foreground">
            Requires Rust 1.80+ and (on Linux) <code className="font-mono">libfontconfig1-dev</code> plus X11/Wayland headers.
          </p>
          <pre className="mt-3 overflow-x-auto rounded-lg border border-fd-border bg-fd-secondary/50 p-3.5 font-mono text-xs leading-6 text-fd-foreground">
            <code>{`git clone https://github.com/azocloud/azo-type.git
cd azo-type
cargo build --release --manifest-path app/Cargo.toml
./app/target/release/azo-type`}</code>
          </pre>
        </div>
      </section>
    </main>
  );
}
