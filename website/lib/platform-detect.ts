import type { ArchId, PlatformId } from './releases';

export interface HostNavigatorInput {
  userAgent: string;
  platform: string;
  maxTouchPoints: number;
  uaDataPlatform?: string;
  highEntropyArch?: string;
  highEntropyBitness?: string;
  highEntropyPlatformVersion?: string;
}

export interface DetectedHostSystem {
  os: PlatformId | 'mobile' | 'unknown';
  arch: ArchId | null;
  osLabel: string;
  archLabel: string | null;
  osMajorVersion: number | null;
  confidentArch: boolean;
}

interface NavigatorUADataLike {
  platform?: string;
  getHighEntropyValues?: (hints: string[]) => Promise<{
    architecture?: string;
    bitness?: string;
    platformVersion?: string;
  }>;
}

function parseMajorVersion(raw?: string): number | null {
  if (!raw) return null;
  const match = raw.trim().match(/^(\d+)/);
  if (!match) return null;
  const parsed = Number.parseInt(match[1], 10);
  return Number.isFinite(parsed) ? parsed : null;
}

export function formatArchLabel(
  os: PlatformId | 'mobile' | 'unknown',
  arch: ArchId | null,
): string | null {
  if (!arch) return null;
  if (os === 'macos') {
    return arch === 'arm64' ? 'Apple Silicon (arm64)' : 'Intel (x86_64)';
  }
  if (arch === 'arm64') {
    return 'ARM64 (aarch64)';
  }
  return 'x86_64 (64-bit)';
}

export function detectHostFromHints(
  input: HostNavigatorInput,
): DetectedHostSystem {
  const ua = input.userAgent || '';
  const navPlatform = input.platform || '';
  const uaDataPlatform = (input.uaDataPlatform || '').trim();
  const osMajorVersion = parseMajorVersion(input.highEntropyPlatformVersion);

  // Exclude mobile and tablet browsers so desktop binaries are not highlighted for phones/tablets.
  if (/^android$/i.test(uaDataPlatform) || /Android/i.test(ua)) {
    return {
      os: 'mobile',
      arch: null,
      osLabel: 'Android',
      archLabel: null,
      osMajorVersion,
      confidentArch: false,
    };
  }

  if (
    /^ios$/i.test(uaDataPlatform) ||
    /iPhone|iPad|iPod/i.test(ua) ||
    (/Mac/i.test(navPlatform) && input.maxTouchPoints > 1)
  ) {
    return {
      os: 'mobile',
      arch: null,
      osLabel: 'iOS / iPadOS',
      archLabel: null,
      osMajorVersion,
      confidentArch: false,
    };
  }

  if (/chrome\s*os/i.test(uaDataPlatform) || /CrOS/i.test(ua)) {
    return {
      os: 'unknown',
      arch: null,
      osLabel: 'ChromeOS',
      archLabel: null,
      osMajorVersion,
      confidentArch: false,
    };
  }

  let os: PlatformId | 'unknown' = 'unknown';
  let osLabel = 'Unknown OS';

  if (
    /^win/i.test(uaDataPlatform) ||
    /Windows|Win64|Win32|WOW64/i.test(ua) ||
    /^Win/i.test(navPlatform)
  ) {
    os = 'windows';
    osLabel = 'Windows';
  } else if (
    /^mac/i.test(uaDataPlatform) ||
    /Macintosh|Mac OS X/i.test(ua) ||
    /^Mac/i.test(navPlatform)
  ) {
    os = 'macos';
    osLabel = 'macOS';
  } else if (
    /^linux/i.test(uaDataPlatform) ||
    /Linux|X11/i.test(ua) ||
    /Linux/i.test(navPlatform)
  ) {
    os = 'linux';
    osLabel = 'Linux';
  }

  if (os === 'unknown') {
    return {
      os: 'unknown',
      arch: null,
      osLabel,
      archLabel: null,
      osMajorVersion,
      confidentArch: false,
    };
  }

  let arch: ArchId | null = null;
  let confidentArch = false;

  const heArch = (input.highEntropyArch || '').trim().toLowerCase();
  if (heArch === 'arm') {
    arch = 'arm64';
    confidentArch = true;
  } else if (heArch === 'x86') {
    arch = 'x86_64';
    confidentArch = true;
  } else if (os === 'windows') {
    if (/(arm64|aarch64)/i.test(ua) || /(arm64|aarch64)/i.test(navPlatform)) {
      arch = 'arm64';
      confidentArch = true;
    } else if (/(win64|x64|x86_64|amd64|wow64)/i.test(ua)) {
      arch = 'x86_64';
      confidentArch = true;
    }
  } else if (os === 'macos') {
    // Note: Safari and Firefox intentionally freeze navigator.platform to "MacIntel"
    // and UA to "Intel Mac OS X" even on Apple Silicon Macs, so only trust explicit
    // ARM hints when User-Agent Client Hints are unavailable.
    if (
      /(arm64|aarch64|macarm)/i.test(ua) ||
      /(arm64|aarch64|macarm)/i.test(navPlatform)
    ) {
      arch = 'arm64';
      confidentArch = true;
    }
  } else if (os === 'linux') {
    if (/(aarch64|arm64|armv8)/i.test(ua) || /(aarch64|arm64)/i.test(navPlatform)) {
      arch = 'arm64';
      confidentArch = true;
    } else if (
      /(x86_64|x86-64|amd64|x64)/i.test(ua) ||
      /(x86_64|amd64)/i.test(navPlatform)
    ) {
      arch = 'x86_64';
      confidentArch = true;
    }
  }

  return {
    os,
    arch,
    osLabel,
    archLabel: formatArchLabel(os, arch),
    osMajorVersion,
    confidentArch,
  };
}

function getBaseNavigatorHints(): HostNavigatorInput {
  if (typeof navigator === 'undefined') {
    return { userAgent: '', platform: '', maxTouchPoints: 0 };
  }
  const nav = navigator as Navigator & { userAgentData?: NavigatorUADataLike };
  return {
    userAgent: nav.userAgent || '',
    platform: nav.platform || '',
    maxTouchPoints: nav.maxTouchPoints || 0,
    uaDataPlatform: nav.userAgentData?.platform,
  };
}

export function detectHostSystemSync(): DetectedHostSystem {
  return detectHostFromHints(getBaseNavigatorHints());
}

export async function detectHostSystem(): Promise<DetectedHostSystem> {
  const base = getBaseNavigatorHints();
  if (typeof navigator === 'undefined') {
    return detectHostFromHints(base);
  }

  const nav = navigator as Navigator & { userAgentData?: NavigatorUADataLike };
  if (typeof nav.userAgentData?.getHighEntropyValues === 'function') {
    try {
      const hints = await nav.userAgentData.getHighEntropyValues([
        'architecture',
        'bitness',
        'platformVersion',
      ]);
      return detectHostFromHints({
        ...base,
        highEntropyArch: hints.architecture,
        highEntropyBitness: hints.bitness,
        highEntropyPlatformVersion: hints.platformVersion,
      });
    } catch {
      return detectHostFromHints(base);
    }
  }

  return detectHostFromHints(base);
}
