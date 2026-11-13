//! Unit tests for the hardcoded Typst release catalog and pure-Rust SHA-256 verification.

use super::*;

#[test]
fn test_sha256_known_nist_test_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert!(verify_sha256_hex(
        b"abc",
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
    ));
    assert!(!verify_sha256_hex(
        b"abd",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    ));
}

#[test]
fn test_hardcoded_catalog_contains_valid_checksums_and_targets() {
    assert!(!AVAILABLE_TYPST_RELEASES.is_empty());
    let expected_targets = [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ];

    for release in AVAILABLE_TYPST_RELEASES {
        assert!(!release.version.is_empty());
        assert!(!is_builtin_version(release.version));
        for target in expected_targets {
            let asset = find_asset_for_target(release.version, target)
                .expect("every catalog release must cover all desktop targets");
            assert!(asset
                .download_url
                .starts_with("https://github.com/typst/typst/releases/"));
            assert_eq!(asset.sha256_hex.len(), 64);
            assert!(asset.sha256_hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    assert!(is_builtin_version(BUILTIN_TYPST_VERSION));
    assert_eq!(all_selectable_versions()[0], BUILTIN_TYPST_VERSION);
}
