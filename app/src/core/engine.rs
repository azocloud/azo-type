//! Typst engine release catalog loaded from embedded JSON, target matching, and pure-Rust SHA-256 checks.

use std::sync::OnceLock;

#[cfg(test)]
mod tests;

/// Built-in Typst compiler version bundled directly via Cargo dependencies.
pub const BUILTIN_TYPST_VERSION: &str = "0.15.1";

const TYPST_RELEASES_JSON: &str = include_str!("../../assets/typst_releases.json");

/// Archive container format used for downloadable Typst release packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineArchiveKind {
    /// XZ-compressed TAR archive (`.tar.xz`, standard on Linux and macOS).
    TarXz,
    /// ZIP archive (`.zip`, standard on Windows).
    Zip,
}

/// Platform-specific downloadable archive metadata and expected SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct EngineReleaseAsset {
    /// Target triple matching official Typst release naming (e.g. `"x86_64-unknown-linux-musl"`).
    pub target_triple: String,
    /// Direct HTTPS download URL on official Typst GitHub releases.
    pub download_url: String,
    /// Expected lowercase 64-character hexadecimal SHA-256 checksum of the compressed archive.
    pub sha256_hex: String,
    /// Archive container format.
    pub archive_kind: EngineArchiveKind,
    /// Executable filename extracted into the local engine cache directory.
    pub binary_name: String,
}

/// Release descriptor for an on-demand downloadable legacy Typst version.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct TypstEngineRelease {
    /// Semantic version string (e.g. `"0.14.2"`).
    pub version: String,
    /// Per-target release artifacts with verified SHA-256 digests.
    pub assets: Vec<EngineReleaseAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct TypstReleasesManifest {
    schema_version: u32,
    releases: Vec<TypstEngineRelease>,
}

static TYPST_RELEASES_CACHE: OnceLock<Vec<TypstEngineRelease>> = OnceLock::new();

/// Returns the catalog of downloadable legacy Typst releases parsed from the embedded JSON manifest.
pub fn available_typst_releases() -> &'static [TypstEngineRelease] {
    TYPST_RELEASES_CACHE
        .get_or_init(|| {
            let manifest: TypstReleasesManifest = serde_json::from_str(TYPST_RELEASES_JSON)
                .expect("embedded typst_releases.json must be valid JSON");
            assert_eq!(
                manifest.schema_version, 1,
                "unsupported typst_releases.json schema_version"
            );
            manifest.releases
        })
        .as_slice()
}

/// Returns whether `version` identifies the built-in bundled Typst compiler.
pub fn is_builtin_version(version: &str) -> bool {
    version.trim() == BUILTIN_TYPST_VERSION
}

/// Returns the Typst release target triple for the compiled host OS and CPU architecture.
pub fn host_target_triple() -> Option<&'static str> {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Some("x86_64-unknown-linux-musl")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Some("aarch64-unknown-linux-musl")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Some("x86_64-apple-darwin")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Some("aarch64-apple-darwin")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        Some("x86_64-pc-windows-msvc")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "aarch64") {
        Some("aarch64-pc-windows-msvc")
    } else {
        None
    }
}

/// Looks up a legacy release descriptor by version string.
pub fn find_release_for_version(version: &str) -> Option<&'static TypstEngineRelease> {
    available_typst_releases()
        .iter()
        .find(|rel| rel.version == version.trim())
}

/// Finds the downloadable release asset for `version` matching a specific `target_triple`.
pub fn find_asset_for_target(
    version: &str,
    target_triple: &str,
) -> Option<&'static EngineReleaseAsset> {
    find_release_for_version(version)?
        .assets
        .iter()
        .find(|asset| asset.target_triple == target_triple)
}

/// Finds the downloadable release asset for `version` matching the current host platform.
pub fn find_host_asset(version: &str) -> Option<&'static EngineReleaseAsset> {
    let target = host_target_triple()?;
    find_asset_for_target(version, target)
}

/// Returns all selectable Typst version identifiers (`"0.15.1"` first, followed by legacy versions).
pub fn all_selectable_versions() -> Vec<&'static str> {
    let releases = available_typst_releases();
    let mut versions = Vec::with_capacity(1 + releases.len());
    versions.push(BUILTIN_TYPST_VERSION);
    for release in releases {
        versions.push(release.version.as_str());
    }
    versions
}

/// Computes the lowercase 64-character hex SHA-256 digest of `data` using pure Rust (FIPS 180-4).
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, chunk) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*chunk);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        let vars = [a, b, c, d, e, f, g, h];
        for (s, v) in state.iter_mut().zip(vars) {
            *s = s.wrapping_add(v);
        }
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{word:08x}");
    }
    hex
}

/// Verifies that `data`'s SHA-256 digest matches `expected_hex` (case-insensitive).
pub fn verify_sha256_hex(data: &[u8], expected_hex: &str) -> bool {
    sha256_hex(data).eq_ignore_ascii_case(expected_hex.trim())
}
