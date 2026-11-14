import test from 'node:test';
import assert from 'node:assert/strict';
import type * as ReleasesModule from '../lib/releases';
import type * as PlatformDetectModule from '../lib/platform-detect';

const releases = (await import(
  new URL('../lib/releases.ts', import.meta.url).href
)) as typeof ReleasesModule;

const platformDetect = (await import(
  new URL('../lib/platform-detect.ts', import.meta.url).href
)) as typeof PlatformDetectModule;

const SAMPLE_V0_ASSETS = [
  {
    name: 'azo-type-v0.0.0-aarch64-linux.tar.gz',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-aarch64-linux.tar.gz',
    size: 38991936,
    digest:
      'sha256:d8af832f7a74be95a4e700eb6da507338b4c780cc6fbf3ca53b418ecd79b7f25',
  },
  {
    name: 'azo-type-v0.0.0-aarch64-windows.zip',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-aarch64-windows.zip',
    size: 35756442,
    digest:
      'sha256:b25355f70c4409212f1b9ae872870369a2f6d3d1574494d75b4ddd092c121ccc',
  },
  {
    name: 'azo-type-v0.0.0-arm64-macos.tar.gz',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-arm64-macos.tar.gz',
    size: 35546726,
    digest:
      'sha256:19b1fe4a90251a6571b505d9390dc1b198cc95d872e4847f7e116e4fc24b2a4b',
  },
  {
    name: 'azo-type-v0.0.0-x86_64-linux.tar.gz',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-x86_64-linux.tar.gz',
    size: 40475034,
    digest:
      'sha256:1690514e420f88805266ec3b74951a069b42a74ba404d99a8be840983a5efcd7',
  },
  {
    name: 'azo-type-v0.0.0-x86_64-macos.tar.gz',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-x86_64-macos.tar.gz',
    size: 37958451,
    digest:
      'sha256:fa641d5848ccdf8277d518ac048c5e3a52f3f97344bc70b61db161c783da790e',
  },
  {
    name: 'azo-type-v0.0.0-x86_64-windows.zip',
    browser_download_url:
      'https://github.com/azocloud/azo-type/releases/download/v0.0.0/azo-type-v0.0.0-x86_64-windows.zip',
    size: 37014733,
    digest:
      'sha256:81970234bf9f06abb2b94325764d3feb5c22ce19b71462fd58010e41cb9936dc',
  },
];

test('matches all 6 release targets exclusively without cross-OS leakage', () => {
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'linux', 'x86_64')?.name,
    'azo-type-v0.0.0-x86_64-linux.tar.gz',
  );
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'linux', 'arm64')?.name,
    'azo-type-v0.0.0-aarch64-linux.tar.gz',
  );
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'macos', 'arm64')?.name,
    'azo-type-v0.0.0-arm64-macos.tar.gz',
  );
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'macos', 'x86_64')?.name,
    'azo-type-v0.0.0-x86_64-macos.tar.gz',
  );
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'windows', 'x86_64')?.name,
    'azo-type-v0.0.0-x86_64-windows.zip',
  );
  assert.equal(
    releases.matchAssetForTarget(SAMPLE_V0_ASSETS, 'windows', 'arm64')?.name,
    'azo-type-v0.0.0-aarch64-windows.zip',
  );
});

test('matches future versioned filenames and ignores checksums or source archives', () => {
  const futureAssets = [
    {
      name: 'azo-type-v0.4.2-x86_64-macos.tar.gz.sha256',
      browser_download_url:
        'https://github.com/azocloud/azo-type/releases/download/v0.4.2/azo-type-v0.4.2-x86_64-macos.tar.gz.sha256',
      size: 96,
    },
    {
      name: 'azo-type-v0.4.2-x86_64-macos.tar.gz',
      browser_download_url:
        'https://github.com/azocloud/azo-type/releases/download/v0.4.2/azo-type-v0.4.2-x86_64-macos.tar.gz',
      size: 38000000,
    },
    {
      name: 'Source code (zip)',
      browser_download_url:
        'https://github.com/azocloud/azo-type/archive/refs/tags/v0.4.2.zip',
      size: 100000,
    },
  ];

  assert.equal(
    releases.matchAssetForTarget(futureAssets, 'macos', 'x86_64')?.name,
    'azo-type-v0.4.2-x86_64-macos.tar.gz',
  );
  assert.equal(
    releases.matchAssetForTarget(futureAssets, 'linux', 'x86_64'),
    null,
  );
  assert.equal(
    releases.matchAssetForTarget(futureAssets, 'windows', 'x86_64'),
    null,
  );
  assert.deepEqual(
    releases.getExtraAssetsForPlatform(SAMPLE_V0_ASSETS, 'macos'),
    [],
  );
});

test('sanitizeRelease rejects untrusted URLs and drafts', () => {
  const draft = releases.sanitizeRelease({
    tag_name: 'v0.1.0',
    draft: true,
    assets: SAMPLE_V0_ASSETS,
  });
  assert.equal(draft, null);

  const sanitized = releases.sanitizeRelease({
    tag_name: 'v0.1.0',
    name: 'Release v0.1.0',
    html_url: 'https://evil.example.com/phish',
    assets: [
      ...SAMPLE_V0_ASSETS,
      {
        name: 'azo-type-v0.1.0-x86_64-windows.zip',
        browser_download_url: 'https://evil.example.com/payload.zip',
        size: 123,
      },
    ],
  });

  assert.ok(sanitized);
  assert.equal(sanitized.html_url, releases.LATEST_RELEASE_URL);
  assert.equal(sanitized.assets.length, SAMPLE_V0_ASSETS.length);
});

test('formats bytes, sha256 digests, and UTC dates cleanly', () => {
  assert.equal(releases.formatBytes(38991936), '37.2 MB');
  assert.equal(
    releases.formatSha256(
      'sha256:d8af832f7a74be95a4e700eb6da507338b4c780cc6fbf3ca53b418ecd79b7f25',
    )?.short,
    'd8af832f…d79b7f25',
  );
  assert.equal(releases.formatSha256('not-a-hash'), null);
  assert.equal(
    releases.formatReleaseDate('2026-11-14T17:44:45Z'),
    'Nov 14, 2026',
  );
});

test('detectHostFromHints accurately identifies Windows, macOS, Linux, and mobile UAs', () => {
  const chromeWin64 = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36',
    platform: 'Win32',
    maxTouchPoints: 0,
    uaDataPlatform: 'Windows',
    highEntropyArch: 'x86',
    highEntropyBitness: '64',
    highEntropyPlatformVersion: '15.0.0',
  });
  assert.deepEqual(chromeWin64, {
    os: 'windows',
    arch: 'x86_64',
    osLabel: 'Windows',
    archLabel: 'x86_64 (64-bit)',
    osMajorVersion: 15,
    confidentArch: true,
  });

  const chromeWinArm = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36',
    platform: 'Win32',
    maxTouchPoints: 0,
    uaDataPlatform: 'Windows',
    highEntropyArch: 'arm',
    highEntropyBitness: '64',
  });
  assert.equal(chromeWinArm.os, 'windows');
  assert.equal(chromeWinArm.arch, 'arm64');
  assert.equal(chromeWinArm.confidentArch, true);

  const chromeMacVenturaIntel = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36',
    platform: 'MacIntel',
    maxTouchPoints: 0,
    uaDataPlatform: 'macOS',
    highEntropyArch: 'x86',
    highEntropyPlatformVersion: '13.6.7',
  });
  assert.equal(chromeMacVenturaIntel.os, 'macos');
  assert.equal(chromeMacVenturaIntel.arch, 'x86_64');
  assert.equal(chromeMacVenturaIntel.osMajorVersion, 13);
  assert.equal(chromeMacVenturaIntel.confidentArch, true);

  // Safari on Apple Silicon freezes UA/platform to Intel: must not falsely recommend x86_64.
  const safariMac = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15',
    platform: 'MacIntel',
    maxTouchPoints: 0,
  });
  assert.equal(safariMac.os, 'macos');
  assert.equal(safariMac.arch, null);
  assert.equal(safariMac.confidentArch, false);

  const firefoxLinux = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0',
    platform: 'Linux x86_64',
    maxTouchPoints: 0,
  });
  assert.equal(firefoxLinux.os, 'linux');
  assert.equal(firefoxLinux.arch, 'x86_64');
  assert.equal(firefoxLinux.confidentArch, true);

  const ipadSafari = platformDetect.detectHostFromHints({
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15',
    platform: 'MacIntel',
    maxTouchPoints: 5,
  });
  assert.equal(ipadSafari.os, 'mobile');
  assert.equal(ipadSafari.arch, null);
});
