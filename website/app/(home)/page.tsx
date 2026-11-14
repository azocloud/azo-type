import Image from 'next/image';
import Link from 'next/link';
import {
  ArrowRight,
  BookOpen,
  Download,
  FileCheck2,
  GitBranch,
  Languages,
  ListTree,
  Network,
  Sparkles,
  Terminal,
  Zap,
} from 'lucide-react';
import screenshotImg from '@/assets/screenshot.png';

const features = [
  {
    icon: Zap,
    title: 'Live Incremental Preview',
    description:
      'Compiles Typst 0.15.1 in a non-blocking background thread with configurable debouncing, 1.5x–3.0x DPI rasterization, Fit Width zoom, and click-to-jump source synchronization.',
  },
  {
    icon: Network,
    title: 'Zero-Server P2P Collaboration',
    description:
      'Real-time multi-user editing powered by Yrs CRDT and Iroh QUIC transport. Share encrypted typst-collab-v1: invite tickets with live peer cursors and isolated local undo/redo.',
  },
  {
    icon: ListTree,
    title: 'Hierarchical AST Outline',
    description:
      'Parses Typst headings, #heading(...) calls, recursive #include/#module files, Markdown sections, and BibTeX @entries with automatic numbering and cursor tracking.',
  },
  {
    icon: FileCheck2,
    title: 'PDF & Archival Standards',
    description:
      'Export to PDF 1.4–2.0, archival ISO 19005 PDF/A (1a/1b through 4/4f/4e), PDF/UA-1 accessibility, and tagged structure trees with live compatibility validation.',
  },
  {
    icon: Languages,
    title: 'Offline Spell & Grammar Check',
    description:
      'Privacy-first proofreading via Hunspell (spellbook), nlprule grammar rules, and Lingua language detection—with on-demand EN, DE, FR, ES packs and custom .dic/.aff support.',
  },
  {
    icon: Sparkles,
    title: 'Native Cross-Platform IDE',
    description:
      'Built in Rust with egui/eframe 0.31 for Linux (Wayland & X11), macOS (native screen-top menu bar), and Windows—complete with light/dark themes and English/German localization.',
  },
];

const techBadges = [
  'Rust 2021',
  'egui / eframe 0.31',
  'Typst 0.15.1',
  'Yrs 0.28 CRDT',
  'Iroh 1.4 QUIC',
  'GPL-3.0',
];

export default function HomePage() {
  return (
    <main className="flex flex-1 flex-col">
      {/* Hero Section */}
      <section className="mx-auto flex w-full max-w-6xl flex-col items-center px-6 pt-16 pb-14 text-center md:pt-24 md:pb-20">
        <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-fd-border bg-fd-card px-3.5 py-1 text-xs font-medium text-fd-muted-foreground">
          <span className="size-2 rounded-full bg-emerald-500" aria-hidden="true" />
          <span>v0.3.0 · Early Pre-Alpha · Open Source (GPL-3.0)</span>
        </div>

        <h1 className="max-w-3xl text-4xl font-bold tracking-tight text-fd-foreground sm:text-5xl md:text-6xl">
          Native Typst authoring, built for speed and P2P collaboration.
        </h1>

        <p className="mt-5 max-w-2xl text-base leading-relaxed text-fd-muted-foreground sm:text-lg">
          <strong>azo-type</strong> is a cross-platform Rust desktop IDE for{' '}
          <a
            href="https://typst.app/"
            target="_blank"
            rel="noreferrer"
            className="font-medium text-fd-foreground underline decoration-fd-border underline-offset-4 hover:decoration-fd-primary"
          >
            Typst
          </a>{' '}
          featuring live incremental visual previews, deep AST outline navigation, standards-compliant PDF/A export, and zero-server collaborative editing.
        </p>

        <div className="mt-8 flex flex-wrap items-center justify-center gap-3">
          <Link
            href="/download"
            className="inline-flex items-center gap-2 rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
          >
            <Download className="size-4" aria-hidden="true" />
            <span>Download</span>
          </Link>
          <Link
            href="/docs"
            className="inline-flex items-center gap-2 rounded-lg border border-fd-border bg-fd-card px-5 py-2.5 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
          >
            <BookOpen className="size-4" aria-hidden="true" />
            <span>Read the Docs</span>
            <ArrowRight className="size-3.5" aria-hidden="true" />
          </Link>
          <a
            href="https://github.com/azocloud/azo-type"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 rounded-lg border border-fd-border px-4 py-2.5 text-sm font-medium text-fd-muted-foreground transition-colors hover:bg-fd-accent hover:text-fd-foreground"
          >
            <GitBranch className="size-4" aria-hidden="true" />
            <span>GitHub</span>
          </a>
        </div>

        <div className="mt-8 flex flex-wrap items-center justify-center gap-2">
          {techBadges.map((badge) => (
            <span
              key={badge}
              className="rounded-md border border-fd-border bg-fd-secondary/60 px-2.5 py-1 font-mono text-xs text-fd-muted-foreground"
            >
              {badge}
            </span>
          ))}
        </div>

        {/* Desktop App Screenshot Preview */}
        <div className="mt-12 w-full overflow-hidden rounded-xl border border-fd-border bg-fd-card shadow-lg">
          <div className="flex items-center justify-between border-b border-fd-border bg-fd-secondary/50 px-4 py-2.5 text-xs text-fd-muted-foreground">
            <div className="flex items-center gap-1.5" aria-hidden="true">
              <span className="size-2.5 rounded-full bg-fd-border" />
              <span className="size-2.5 rounded-full bg-fd-border" />
              <span className="size-2.5 rounded-full bg-fd-border" />
            </div>
            <span className="font-mono">Azo Type — Desktop Workspace</span>
            <span className="hidden sm:inline font-mono">Typst 0.15.1</span>
          </div>
          <Image
            src={screenshotImg}
            alt="Screenshot of the Azo Type desktop IDE showing the project file tree, document outline, syntax-highlighted Typst editor, and live compiled document preview."
            className="h-auto w-full object-cover"
            priority
          />
        </div>
      </section>

      {/* Feature Highlights */}
      <section className="border-t border-fd-border bg-fd-secondary/20 py-16 md:py-20">
        <div className="mx-auto w-full max-w-6xl px-6">
          <div className="mb-10 max-w-2xl">
            <h2 className="text-2xl font-bold tracking-tight text-fd-foreground sm:text-3xl">
              Everything you need for local-first Typst workflows
            </h2>
            <p className="mt-2 text-sm text-fd-muted-foreground sm:text-base">
              Designed around pure domain separation, non-blocking background workers, and native desktop integration.
            </p>
          </div>

          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
            {features.map((feature) => {
              const Icon = feature.icon;
              return (
                <div
                  key={feature.title}
                  className="flex flex-col rounded-xl border border-fd-border bg-fd-card p-6 transition-colors hover:border-fd-primary/40"
                >
                  <div className="mb-4 inline-flex size-9 items-center justify-center rounded-lg border border-fd-border bg-fd-secondary text-fd-foreground">
                    <Icon className="size-4" aria-hidden="true" />
                  </div>
                  <h3 className="text-base font-semibold text-fd-foreground">
                    {feature.title}
                  </h3>
                  <p className="mt-2 text-sm leading-relaxed text-fd-muted-foreground">
                    {feature.description}
                  </p>
                </div>
              );
            })}
          </div>
        </div>
      </section>

      {/* Quick-start & Open Source Callout */}
      <section className="border-t border-fd-border py-16">
        <div className="mx-auto grid w-full max-w-6xl gap-8 px-6 lg:grid-cols-2 lg:items-center">
          <div>
            <div className="inline-flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-fd-muted-foreground">
              <Terminal className="size-3.5" aria-hidden="true" />
              <span>Quick Start from Source</span>
            </div>
            <h2 className="mt-2 text-2xl font-bold tracking-tight text-fd-foreground sm:text-3xl">
              Run locally in seconds with Cargo
            </h2>
            <p className="mt-3 text-sm leading-relaxed text-fd-muted-foreground sm:text-base">
              Launch straight into an in-memory blank Typst document, open an existing project folder from disk, or pass a directory directly via CLI.
            </p>
            <div className="mt-6 flex flex-wrap gap-3">
              <Link
                href="/docs/installation"
                className="inline-flex items-center gap-1.5 rounded-lg border border-fd-border bg-fd-card px-4 py-2 text-sm font-medium text-fd-foreground hover:bg-fd-accent"
              >
                <span>Installation Guide</span>
                <ArrowRight className="size-3.5" aria-hidden="true" />
              </Link>
              <Link
                href="/docs/getting-started"
                className="inline-flex items-center gap-1.5 rounded-lg border border-fd-border bg-fd-card px-4 py-2 text-sm font-medium text-fd-foreground hover:bg-fd-accent"
              >
                <span>Getting Started</span>
                <ArrowRight className="size-3.5" aria-hidden="true" />
              </Link>
            </div>
          </div>

          <div className="overflow-hidden rounded-xl border border-fd-border bg-fd-card">
            <div className="border-b border-fd-border bg-fd-secondary/50 px-4 py-2 font-mono text-xs text-fd-muted-foreground">
              bash
            </div>
            <pre className="overflow-x-auto p-4 font-mono text-xs leading-6 text-fd-foreground sm:text-sm">
              <code>{`git clone https://github.com/azocloud/azo-type.git
cd azo-type

# Run the desktop IDE (opens an in-memory blank project)
cargo run --manifest-path app/Cargo.toml

# Or open an existing Typst project folder directly
cargo run --manifest-path app/Cargo.toml -- ./my-paper`}</code>
            </pre>
          </div>
        </div>
      </section>
    </main>
  );
}
