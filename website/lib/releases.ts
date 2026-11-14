export type PlatformId = 'linux' | 'macos' | 'windows';
export type ArchId = 'x86_64' | 'arm64';

export interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
  digest?: string | null;
}

export interface GitHubRelease {
  tag_name: string;
  name: string;
  html_url: string;
  published_at?: string;
  prerelease?: boolean;
  assets: GitHubReleaseAsset[];
}

export interface ArchTargetSpec {
  arch: ArchId;
  label: string;
  shortLabel: string;
  detail: string;
  requirementHint: string | null;
}

export interface PlatformSpec {
  id: PlatformId;
  name: string;
  architectures: string;
  notes: string;
  requirements: string[];
  targets: ArchTargetSpec[];
}

export const GITHUB_OWNER = 'azocloud';
export const GITHUB_REPO = 'azo-type';
export const RELEASES_URL = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/releases`;
export const LATEST_RELEASE_URL = `${RELEASES_URL}/latest`;
export const GITHUB_API_LATEST = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest`;
export const GITHUB_API_RELEASES = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases?per_page=5`;

const TRUSTED_DOWNLOAD_PREFIX = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/`;

export const PLATFORM_SPECS: PlatformSpec[] = [
  {
    id: 'linux',
    name: 'Linux',
    architectures: 'x86_64 · aarch64 (Wayland & X11)',
    notes:
      'In-window desktop menu bar, GTK/portal native file dialogs via rfd, and hardware-accelerated OpenGL (glow) viewport.',
    requirements: [
      'Wayland or X11 display server',
      'fontconfig runtime & OpenGL drivers',
      'Archive (.tar.gz) includes azo-type binary, azo-type.desktop, and LICENSE',
    ],
    targets: [
      {
        arch: 'x86_64',
        label: 'Linux x86_64',
        shortLabel: 'x86_64',
        detail: '64-bit Intel / AMD · .tar.gz',
        requirementHint: null,
      },
      {
        arch: 'arm64',
        label: 'Linux ARM64',
        shortLabel: 'aarch64',
        detail: '64-bit ARM (aarch64) · .tar.gz',
        requirementHint: null,
      },
    ],
  },
  {
    id: 'macos',
    name: 'macOS',
    architectures: 'Apple Silicon (arm64) · Intel (x86_64)',
    notes:
      'Integrates a native macOS screen-top menu bar via muda, Cmd-based keyboard shortcuts, and native NSOpenPanel / NSSavePanel dialogs.',
    requirements: [
      'macOS 14 (Sonoma) or newer (required for Intel x86_64 and Apple Silicon)',
      'Ad-hoc signed Azo Type.app bundle inside .tar.gz',
      'Right-click → Open on first launch if macOS Gatekeeper prompts',
    ],
    targets: [
      {
        arch: 'arm64',
        label: 'macOS Apple Silicon',
        shortLabel: 'arm64',
        detail: 'M1–M4 Macs · .tar.gz',
        requirementHint: 'macOS 14 Sonoma or newer',
      },
      {
        arch: 'x86_64',
        label: 'macOS Intel',
        shortLabel: 'x86_64',
        detail: 'Intel Macs · .tar.gz',
        requirementHint: 'Requires macOS 14 Sonoma or newer',
      },
    ],
  },
  {
    id: 'windows',
    name: 'Windows',
    architectures: 'x86_64 · aarch64 (MSVC)',
    notes:
      'In-window menu bar, Win32 native file pickers, embedded application icon resources, and Ctrl-based shortcuts.',
    requirements: [
      'Windows 10 or Windows 11 (64-bit)',
      'Microsoft Visual C++ Redistributable & OpenGL-capable GPU driver',
      'Portable .zip archive containing azo-type.exe and LICENSE',
    ],
    targets: [
      {
        arch: 'x86_64',
        label: 'Windows x86_64',
        shortLabel: 'x86_64',
        detail: '64-bit Intel / AMD · .zip',
        requirementHint: null,
      },
      {
        arch: 'arm64',
        label: 'Windows ARM64',
        shortLabel: 'aarch64',
        detail: 'Snapdragon / ARM64 PCs · .zip',
        requirementHint: null,
      },
    ],
  },
];

export function isTrustedGitHubUrl(url: unknown): url is string {
  return typeof url === 'string' && url.startsWith(TRUSTED_DOWNLOAD_PREFIX);
}

export function sanitizeRelease(raw: unknown): GitHubRelease | null {
  if (!raw || typeof raw !== 'object') return null;
  const candidate = raw as Record<string, unknown>;
  if (typeof candidate.tag_name !== 'string' || !candidate.tag_name.trim()) {
    return null;
  }
  if (candidate.draft === true) return null;

  const rawAssets = Array.isArray(candidate.assets) ? candidate.assets : [];
  const assets: GitHubReleaseAsset[] = [];

  for (const item of rawAssets) {
    if (!item || typeof item !== 'object') continue;
    const asset = item as Record<string, unknown>;
    if (
      typeof asset.name === 'string' &&
      isTrustedGitHubUrl(asset.browser_download_url) &&
      typeof asset.size === 'number' &&
      Number.isFinite(asset.size) &&
      asset.size >= 0
    ) {
      assets.push({
        name: asset.name,
        browser_download_url: asset.browser_download_url,
        size: asset.size,
        digest: typeof asset.digest === 'string' ? asset.digest : null,
      });
    }
  }

  return {
    tag_name: candidate.tag_name,
    name: typeof candidate.name === 'string' && candidate.name.trim()
      ? candidate.name
      : candidate.tag_name,
    html_url: isTrustedGitHubUrl(candidate.html_url)
      ? candidate.html_url
      : LATEST_RELEASE_URL,
    published_at:
      typeof candidate.published_at === 'string'
        ? candidate.published_at
        : undefined,
    prerelease: candidate.prerelease === true,
    assets,
  };
}

function isIgnoredAsset(name: string): boolean {
  return (
    /\.(sha256|sha512|asc|sig|txt|json)$/i.test(name) ||
    /^source(\s+code)?/i.test(name)
  );
}

export function matchesAssetPlatform(
  name: string,
  platform: PlatformId,
): boolean {
  if (isIgnoredAsset(name)) return false;

  const hasLinux = /(^|[-_.\s])linux([-_.\s]|$)/i.test(name);
  const hasMacos = /(^|[-_.\s])(macos|darwin|apple)([-_.\s]|$)/i.test(name);
  const hasWindows = /(^|[-_.\s])(windows|win64|win32|msvc)([-_.\s]|$)/i.test(
    name,
  );

  if (hasLinux || hasMacos || hasWindows) {
    if (platform === 'linux') return hasLinux && !hasMacos && !hasWindows;
    if (platform === 'macos') return hasMacos && !hasLinux && !hasWindows;
    return hasWindows && !hasLinux && !hasMacos;
  }

  if (platform === 'linux') return /\.(AppImage|deb|rpm)$/i.test(name);
  if (platform === 'macos') return /\.(dmg|pkg)$/i.test(name);
  return /\.(exe|msi)$/i.test(name);
}

export function matchesAssetArch(name: string, arch: ArchId): boolean {
  if (arch === 'x86_64') {
    return /(^|[-_.\s])(x86_64|amd64|x64)([-_.\s]|$)/i.test(name);
  }
  return /(^|[-_.\s])(aarch64|arm64)([-_.\s]|$)/i.test(name);
}

export function matchAssetForTarget(
  assets: GitHubReleaseAsset[],
  platform: PlatformId,
  arch: ArchId,
): GitHubReleaseAsset | null {
  return (
    assets.find(
      (asset) =>
        matchesAssetPlatform(asset.name, platform) &&
        matchesAssetArch(asset.name, arch),
    ) ?? null
  );
}

export function getExtraAssetsForPlatform(
  assets: GitHubReleaseAsset[],
  platform: PlatformId,
): GitHubReleaseAsset[] {
  const matchedNames = new Set(
    (['x86_64', 'arm64'] as const)
      .map((arch) => matchAssetForTarget(assets, platform, arch)?.name)
      .filter((name): name is string => Boolean(name)),
  );
  return assets.filter(
    (asset) =>
      matchesAssetPlatform(asset.name, platform) && !matchedNames.has(asset.name),
  );
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatSha256(digest?: string | null): {
  full: string;
  short: string;
} | null {
  if (!digest) return null;
  const normalized = digest.replace(/^sha256:/i, '').trim();
  if (!/^[a-f0-9]{64}$/i.test(normalized)) return null;
  return {
    full: normalized,
    short: `${normalized.slice(0, 8)}…${normalized.slice(-8)}`,
  };
}

export function formatReleaseDate(iso?: string): string | null {
  if (!iso) return null;
  const timestamp = Date.parse(iso);
  if (Number.isNaN(timestamp)) return null;
  return new Intl.DateTimeFormat('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }).format(new Date(timestamp));
}

export interface FetchReleaseOptions {
  isClient?: boolean;
  token?: string;
  priority?: 'low' | 'auto';
  signal?: AbortSignal;
  timeoutMs?: number;
}

export async function fetchLatestRelease(
  options: FetchReleaseOptions = {},
): Promise<GitHubRelease | null> {
  const {
    isClient = false,
    token,
    priority,
    signal,
    timeoutMs = 4500,
  } = options;

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
  const onExternalAbort = () => controller.abort();
  signal?.addEventListener('abort', onExternalAbort, { once: true });

  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
  };
  if (!isClient) {
    headers['User-Agent'] = 'azo-type-website';
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }
  }

  const init: RequestInit & { priority?: 'low' | 'auto' | 'high' } = {
    headers,
    signal: controller.signal,
  };
  if (isClient) {
    init.cache = 'no-store';
    if (priority) {
      init.priority = priority;
    }
  }

  try {
    const res = await fetch(GITHUB_API_LATEST, init);
    if (res.ok) {
      const parsed = sanitizeRelease(await res.json());
      if (parsed && parsed.assets.length > 0) {
        return parsed;
      }
    }

    // If /releases/latest returns 404 (e.g. only prereleases exist), inspect recent releases.
    if (res.status === 404) {
      const listRes = await fetch(GITHUB_API_RELEASES, init);
      if (listRes.ok) {
        const list = (await listRes.json()) as unknown;
        if (Array.isArray(list)) {
          for (const item of list) {
            const parsed = sanitizeRelease(item);
            if (parsed && parsed.assets.length > 0) {
              return parsed;
            }
          }
        }
      }
    }

    return null;
  } catch {
    return null;
  } finally {
    clearTimeout(timeoutId);
    signal?.removeEventListener('abort', onExternalAbort);
  }
}
