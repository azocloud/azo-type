'use client';

import { useEffect, useState } from 'react';
import {
  AlertTriangle,
  ArrowUpRight,
  Check,
  CheckCircle2,
  Copy,
  Download,
  Laptop,
  Sparkles,
} from 'lucide-react';
import {
  type DetectedHostSystem,
  detectHostSystem,
  detectHostSystemSync,
} from '@/lib/platform-detect';
import {
  type GitHubRelease,
  LATEST_RELEASE_URL,
  PLATFORM_SPECS,
  type PlatformSpec,
  formatBytes,
  formatReleaseDate,
  formatSha256,
  getExtraAssetsForPlatform,
  matchAssetForTarget,
} from '@/lib/releases';

interface DownloadCatalogProps {
  release: GitHubRelease | null;
}

function RecommendedSpotlight({
  detectedHost,
  release,
}: {
  detectedHost: DetectedHostSystem | null;
  release: GitHubRelease | null;
}) {
  const releaseHref = release?.html_url ?? LATEST_RELEASE_URL;

  if (!detectedHost) {
    return (
      <section
        aria-labelledby="recommended-download-heading"
        className="mt-6 rounded-xl border border-fd-border bg-fd-card p-5"
      >
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-fd-border bg-fd-secondary text-fd-foreground">
              <Laptop className="size-4" aria-hidden="true" />
            </div>
            <div>
              <h2
                id="recommended-download-heading"
                className="text-sm font-semibold text-fd-foreground"
              >
                Platform Downloads
              </h2>
              <p className="mt-0.5 text-xs text-fd-muted-foreground">
                Choose the desktop archive matching your operating system and CPU
                architecture below.
              </p>
            </div>
          </div>
          <a
            href={releaseHref}
            target="_blank"
            rel="noreferrer"
            className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 rounded-lg border border-fd-border bg-fd-secondary/60 px-4 py-2 text-xs font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
          >
            <span>Browse GitHub Release</span>
            <ArrowUpRight className="size-3.5" aria-hidden="true" />
          </a>
        </div>
      </section>
    );
  }

  if (detectedHost.os === 'mobile' || detectedHost.os === 'unknown') {
    return (
      <section
        aria-labelledby="recommended-download-heading"
        className="mt-6 rounded-xl border border-fd-border bg-fd-card p-5"
      >
        <div className="flex items-start gap-3">
          <div className="mt-0.5 inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-fd-border bg-fd-secondary text-fd-foreground">
            <Laptop className="size-4" aria-hidden="true" />
          </div>
          <div>
            <h2
              id="recommended-download-heading"
              className="text-sm font-semibold text-fd-foreground"
            >
              Detected {detectedHost.osLabel}
            </h2>
            <p className="mt-0.5 text-xs leading-relaxed text-fd-muted-foreground">
              Azo Type is a native desktop application for Linux, macOS (Sonoma
              14+), and Windows. Select your target desktop platform below.
            </p>
          </div>
        </div>
      </section>
    );
  }

  const platformSpec: PlatformSpec | undefined = PLATFORM_SPECS.find(
    (item) => item.id === detectedHost.os,
  );
  if (!platformSpec) return null;

  const confidentTarget =
    detectedHost.confidentArch && detectedHost.arch
      ? (platformSpec.targets.find((t) => t.arch === detectedHost.arch) ?? null)
      : null;

  const matchedAsset =
    confidentTarget && release
      ? matchAssetForTarget(
          release.assets,
          platformSpec.id,
          confidentTarget.arch,
        )
      : null;

  const showSonomaWarning =
    detectedHost.os === 'macos' &&
    detectedHost.arch === 'x86_64' &&
    detectedHost.osMajorVersion !== null &&
    detectedHost.osMajorVersion < 14;

  return (
    <section
      aria-labelledby="recommended-download-heading"
      className="download-card-highlighted mt-6 w-full min-w-0 overflow-hidden rounded-xl border border-fd-primary/60 bg-fd-primary/5 p-5"
    >
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div className="min-w-0">
          <div className="inline-flex items-center gap-1.5 rounded-full border border-fd-primary/40 bg-fd-primary/10 px-2.5 py-0.5 text-xs font-medium text-fd-foreground">
            <Sparkles className="size-3.5 text-fd-primary" aria-hidden="true" />
            <span>Recommended for your system</span>
          </div>
          <h2
            id="recommended-download-heading"
            className="mt-2 text-lg font-semibold text-fd-foreground"
          >
            {detectedHost.osLabel}
            {detectedHost.archLabel ? ` · ${detectedHost.archLabel}` : ''}
          </h2>
          {confidentTarget ? (
            <p className="mt-1 text-xs leading-relaxed text-fd-muted-foreground">
              {matchedAsset ? (
                <>
                  Latest asset:{' '}
                  <code className="break-all font-mono text-fd-foreground">
                    {matchedAsset.name}
                  </code>{' '}
                  ({formatBytes(matchedAsset.size)}). Need a different target?
                  All 6 builds are listed below.
                </>
              ) : (
                <>
                  Detected {confidentTarget.label}. Direct release packages are
                  hosted on GitHub Releases.
                </>
              )}
            </p>
          ) : (
            <p className="mt-1 text-xs leading-relaxed text-fd-muted-foreground">
              {detectedHost.os === 'macos'
                ? 'Your browser does not expose CPU architecture—choose Apple Silicon (M1–M4 Macs) or Intel (requires macOS 14 Sonoma or newer).'
                : `Your browser did not report a specific CPU architecture—choose either ${platformSpec.name} build below.`}
            </p>
          )}
          {showSonomaWarning && (
            <p className="mt-2 inline-flex items-center gap-1.5 text-xs font-medium text-amber-700 dark:text-amber-300">
              <AlertTriangle className="size-3.5 shrink-0" aria-hidden="true" />
              <span>
                Your macOS version reports {detectedHost.osMajorVersion}.x; the
                Intel (x86_64) build requires macOS 14 Sonoma or newer.
              </span>
            </p>
          )}
        </div>

        <div className="flex min-w-0 flex-wrap items-center gap-2.5">
          {confidentTarget ? (
            <a
              href={matchedAsset?.browser_download_url ?? releaseHref}
              target="_blank"
              rel="noreferrer"
              className="inline-flex min-h-11 max-w-full items-center gap-2 rounded-lg bg-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
            >
              <Download className="size-4 shrink-0" aria-hidden="true" />
              <span>
                Download {confidentTarget.label}
                {matchedAsset ? ` (${formatBytes(matchedAsset.size)})` : ''}
              </span>
              {!matchedAsset && (
                <ArrowUpRight className="size-4 shrink-0" aria-hidden="true" />
              )}
            </a>
          ) : (
            platformSpec.targets.map((target) => {
              const asset = release
                ? matchAssetForTarget(
                    release.assets,
                    platformSpec.id,
                    target.arch,
                  )
                : null;
              return (
                <a
                  key={target.arch}
                  href={asset?.browser_download_url ?? releaseHref}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex min-h-10 items-center gap-2 rounded-lg bg-fd-primary px-4 py-2 text-xs font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
                >
                  <Download className="size-3.5 shrink-0" aria-hidden="true" />
                  <span>
                    {target.label}
                    {asset ? ` (${formatBytes(asset.size)})` : ''}
                  </span>
                </a>
              );
            })
          )}
        </div>
      </div>
    </section>
  );
}

export function DownloadCatalog({ release }: DownloadCatalogProps) {
  const [detectedHost, setDetectedHost] = useState<DetectedHostSystem | null>(
    null,
  );
  const [copiedDigest, setCopiedDigest] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setDetectedHost(detectHostSystemSync());

    detectHostSystem().then((refined) => {
      if (active) {
        setDetectedHost(refined);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  async function handleCopyChecksum(assetName: string, fullHash: string) {
    try {
      await navigator.clipboard.writeText(fullHash);
      setCopiedDigest(assetName);
      window.setTimeout(() => {
        setCopiedDigest((current) => (current === assetName ? null : current));
      }, 2000);
    } catch {
      // Clipboard API unavailable in restricted contexts; ignore silently.
    }
  }

  const releaseHref = release?.html_url ?? LATEST_RELEASE_URL;
  const publishedDate = formatReleaseDate(release?.published_at);

  return (
    <>
      {/* Alpha Warning & Latest Release Status Banner */}
      <aside
        aria-label="Alpha software notice and latest release status"
        className="mt-8 w-full min-w-0 overflow-hidden rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 text-sm"
      >
        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 items-start gap-3">
            <AlertTriangle
              className="mt-0.5 size-5 shrink-0 text-amber-700 dark:text-amber-400"
              aria-hidden="true"
            />
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <p className="font-semibold text-fd-foreground">
                  Alpha Release Warning
                </p>
                {release && (
                  <span className="rounded-md border border-fd-border bg-fd-card px-2 py-0.5 font-mono text-xs font-medium text-fd-foreground">
                    {release.tag_name}
                  </span>
                )}
                {publishedDate && (
                  <span className="text-xs text-fd-muted-foreground">
                    Published {publishedDate}
                  </span>
                )}
              </div>
              <p className="mt-1 leading-relaxed text-fd-muted-foreground">
                <strong className="text-fd-foreground">Azo Type</strong> is
                currently in{' '}
                <strong className="text-fd-foreground">alpha</strong>{' '}
                development. Pre-built desktop binaries are ready to download
                for Linux, macOS (Sonoma 14+ required), and Windows, though
                bugs, rough edges, and breaking changes may still occur.
              </p>
              {!release && (
                <p className="mt-1.5 text-xs text-fd-muted-foreground">
                  Release asset links were unavailable when this static page was
                  built. Use the links below to open the latest GitHub Release
                  directly.
                </p>
              )}
            </div>
          </div>

          <div className="flex shrink-0 flex-col items-start gap-1.5 sm:items-end">
            <a
              href={releaseHref}
              target="_blank"
              rel="noreferrer"
              className="inline-flex min-h-8 items-center gap-1 text-xs font-medium text-fd-primary hover:underline"
            >
              <span>View release notes</span>
              <ArrowUpRight className="size-3.5" aria-hidden="true" />
            </a>
          </div>
        </div>
      </aside>

      {/* Host-Detected Recommended Download Banner */}
      <RecommendedSpotlight detectedHost={detectedHost} release={release} />

      {/* All 3 Platform Cards (fixed DOM order) */}
      <section
        aria-label="Desktop downloads by operating system"
        className="mt-6 grid w-full min-w-0 grid-cols-1 gap-5 md:grid-cols-3"
      >
        {PLATFORM_SPECS.map((platform) => {
          const isDetectedOs = detectedHost?.os === platform.id;
          const extraAssets = release
            ? getExtraAssetsForPlatform(release.assets, platform.id)
            : [];

          return (
            <article
              key={platform.id}
              className={`flex min-w-0 flex-col justify-between overflow-hidden rounded-xl border p-6 transition-colors ${
                isDetectedOs
                  ? 'download-card-highlighted border-fd-primary bg-fd-card ring-1 ring-fd-primary/30'
                  : 'border-fd-border bg-fd-card'
              }`}
            >
              <div className="min-w-0">
                <div className="flex items-start justify-between gap-2">
                  <h2 className="text-xl font-semibold text-fd-foreground">
                    {platform.name}
                  </h2>
                  {isDetectedOs && (
                    <span className="inline-flex shrink-0 items-center gap-1 rounded-full border border-fd-primary/40 bg-fd-primary/10 px-2.5 py-0.5 text-[11px] font-medium text-fd-foreground">
                      <CheckCircle2
                        className="size-3 text-fd-primary"
                        aria-hidden="true"
                      />
                      <span>Your OS</span>
                    </span>
                  )}
                </div>

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
                        <span
                          className="mt-1.5 size-1 shrink-0 rounded-full bg-fd-muted-foreground"
                          aria-hidden="true"
                        />
                        <span>{req}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              </div>

              <div className="mt-6 min-w-0 space-y-2.5">
                {platform.targets.map((target) => {
                  const asset = release
                    ? matchAssetForTarget(
                        release.assets,
                        platform.id,
                        target.arch,
                      )
                    : null;
                  const isRecommendedTarget =
                    isDetectedOs &&
                    detectedHost?.confidentArch &&
                    detectedHost?.arch === target.arch;
                  const sha256 = formatSha256(asset?.digest);

                  return (
                    <div
                      key={target.arch}
                      className={`min-w-0 overflow-hidden rounded-lg border p-3 transition-colors ${
                        isRecommendedTarget
                          ? 'download-target-highlighted border-fd-primary bg-fd-primary/10'
                          : 'border-fd-border bg-fd-secondary/40'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-semibold text-fd-foreground">
                          {target.label}
                        </span>
                        {isRecommendedTarget ? (
                          <span className="inline-flex items-center gap-1 rounded bg-fd-primary px-1.5 py-0.5 text-[10px] font-semibold text-fd-primary-foreground">
                            <CheckCircle2
                              className="size-3"
                              aria-hidden="true"
                            />
                            <span>Recommended</span>
                          </span>
                        ) : (
                          <span className="font-mono text-[11px] text-fd-muted-foreground">
                            {target.shortLabel}
                          </span>
                        )}
                      </div>

                      <p className="mt-0.5 text-[11px] text-fd-muted-foreground">
                        {target.detail}
                      </p>

                      {target.requirementHint && (
                        <p className="mt-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                          {target.requirementHint}
                        </p>
                      )}

                      <a
                        href={asset ? asset.browser_download_url : releaseHref}
                        target="_blank"
                        rel="noreferrer"
                        className={`mt-2.5 flex min-h-10 w-full min-w-0 items-center justify-between gap-2 rounded-md px-3 py-2 text-xs font-medium transition-colors ${
                          isRecommendedTarget
                            ? 'bg-fd-primary text-fd-primary-foreground hover:opacity-90'
                            : 'border border-fd-border bg-fd-card text-fd-foreground hover:bg-fd-accent'
                        }`}
                      >
                        <span className="inline-flex min-w-0 items-center gap-1.5">
                          <Download
                            className="size-3.5 shrink-0"
                            aria-hidden="true"
                          />
                          <span className="truncate font-mono">
                            {asset
                              ? asset.name
                              : `Download ${target.label} on GitHub`}
                          </span>
                        </span>
                        <span
                          className={`shrink-0 ${
                            isRecommendedTarget
                              ? 'text-fd-primary-foreground/85'
                              : 'text-fd-muted-foreground'
                          }`}
                        >
                          {asset ? (
                            formatBytes(asset.size)
                          ) : (
                            <ArrowUpRight
                              className="size-3.5"
                              aria-hidden="true"
                            />
                          )}
                        </span>
                      </a>

                      {asset && sha256 && (
                        <div className="mt-2 flex items-center justify-between gap-2 text-[11px] text-fd-muted-foreground">
                          <span className="truncate font-mono" title={sha256.full}>
                            sha256: {sha256.short}
                          </span>
                          <button
                            type="button"
                            onClick={() =>
                              handleCopyChecksum(asset.name, sha256.full)
                            }
                            aria-label={`Copy SHA-256 checksum for ${asset.name}`}
                            className="inline-flex min-h-7 shrink-0 items-center gap-1 rounded px-2 py-1 text-[11px] font-medium text-fd-muted-foreground transition-colors hover:bg-fd-accent hover:text-fd-foreground"
                          >
                            {copiedDigest === asset.name ? (
                              <>
                                <Check
                                  className="size-3 text-emerald-500"
                                  aria-hidden="true"
                                />
                                <span>Copied</span>
                              </>
                            ) : (
                              <>
                                <Copy className="size-3" aria-hidden="true" />
                                <span>Copy SHA-256</span>
                              </>
                            )}
                          </button>
                        </div>
                      )}
                    </div>
                  );
                })}

                {extraAssets.map((asset) => (
                  <a
                    key={asset.name}
                    href={asset.browser_download_url}
                    target="_blank"
                    rel="noreferrer"
                    className="flex min-h-10 w-full min-w-0 items-center justify-between gap-2 rounded-lg border border-fd-border bg-fd-secondary/60 px-3.5 py-2 text-xs font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
                  >
                    <span className="min-w-0 truncate font-mono">{asset.name}</span>
                    <span className="ml-2 shrink-0 text-fd-muted-foreground">
                      {formatBytes(asset.size)}
                    </span>
                  </a>
                ))}
              </div>
            </article>
          );
        })}
      </section>
    </>
  );
}
