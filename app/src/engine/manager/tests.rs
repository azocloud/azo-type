//! Unit tests for [`EngineManager`] resolution, fallback behavior, and SHA-256 verification.

use std::fs;
use std::sync::Arc;

use super::archive::verify_and_extract_archive;
use super::{EngineInstallState, EngineManager};
use crate::core::engine::{find_host_asset, EngineArchiveKind, BUILTIN_TYPST_VERSION};
use crate::world::SharedWorldResources;

#[test]
fn test_resolve_builtin_engine_works_offline() {
    let dir = std::env::temp_dir().join(format!("azo-mgr-test-{}", uuid::Uuid::new_v4()));
    let resources = Arc::new(SharedWorldResources::new());
    let mgr = EngineManager::with_paths(resources, dir.clone(), None);

    assert_eq!(
        mgr.install_state(BUILTIN_TYPST_VERSION),
        EngineInstallState::BuiltIn
    );
    let (engine, notice) = mgr.resolve_engine(BUILTIN_TYPST_VERSION);
    let meta = engine.metadata();
    assert_eq!(meta.version, BUILTIN_TYPST_VERSION);
    assert!(meta.is_builtin);
    assert!(notice.is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_missing_or_unknown_version_falls_back_gracefully() {
    let dir = std::env::temp_dir().join(format!("azo-mgr-fb-{}", uuid::Uuid::new_v4()));
    let resources = Arc::new(SharedWorldResources::new());
    let mgr = EngineManager::with_paths(resources, dir.clone(), None);

    let (engine_missing, notice_missing) = mgr.resolve_engine("0.14.2");
    assert!(engine_missing.metadata().is_builtin);
    assert_eq!(engine_missing.metadata().version, BUILTIN_TYPST_VERSION);
    assert!(notice_missing
        .as_deref()
        .is_some_and(|n| n.contains("0.14.2") && n.contains(BUILTIN_TYPST_VERSION)));

    let (engine_unknown, notice_unknown) = mgr.resolve_engine("99.0.0");
    assert!(engine_unknown.metadata().is_builtin);
    assert!(notice_unknown
        .as_deref()
        .is_some_and(|n| n.contains("99.0.0")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_resolve_cached_legacy_binary_activates_external_engine() {
    let dir = std::env::temp_dir().join(format!("azo-mgr-inst-{}", uuid::Uuid::new_v4()));
    if let Some(asset) = find_host_asset("0.14.2") {
        let ver_dir = dir.join("0.14.2");
        fs::create_dir_all(&ver_dir).expect("create version dir");
        fs::write(ver_dir.join(asset.binary_name), b"stub-binary").expect("write stub bin");

        let resources = Arc::new(SharedWorldResources::new());
        let mgr = EngineManager::with_paths(resources, dir.clone(), None);

        assert_eq!(mgr.install_state("0.14.2"), EngineInstallState::Installed);
        let (engine, notice) = mgr.resolve_engine("0.14.2");
        assert!(notice.is_none());
        let meta = engine.metadata();
        assert_eq!(meta.version, "0.14.2");
        assert!(!meta.is_builtin);

        mgr.remove_cached_engine("0.14.2")
            .expect("remove cached engine");
        assert_eq!(
            mgr.install_state("0.14.2"),
            EngineInstallState::NotInstalled
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_verify_and_extract_archive_rejects_corrupted_sha256() {
    let dest = std::env::temp_dir().join(format!("azo-mgr-badsha-{}", uuid::Uuid::new_v4()));
    let err = verify_and_extract_archive(
        "0.14.2",
        b"corrupted payload",
        "0000000000000000000000000000000000000000000000000000000000000000",
        EngineArchiveKind::TarXz,
        "typst",
        &dest,
    )
    .expect_err("checksum mismatch must fail before extraction");

    assert!(err.contains("mismatch") || err.contains("Security check"));
    assert!(!dest.exists(), "destination directory must not be created");
}

#[test]
fn test_apply_download_event_updates_selected_version_and_errors() {
    use super::EngineDownloadEvent;
    use std::path::PathBuf;

    let dir = std::env::temp_dir().join(format!("azo-mgr-ev-{}", uuid::Uuid::new_v4()));
    let resources = Arc::new(SharedWorldResources::new());
    let mgr = EngineManager::with_paths(resources, dir.clone(), None);

    let mut selected = BUILTIN_TYPST_VERSION.to_string();
    let mut fallback = Some("prev fallback".to_string());

    let (_notice_ok, recompile_ok) = mgr.apply_download_event(
        EngineDownloadEvent {
            version: "0.14.2".to_string(),
            result: Ok(PathBuf::from("/tmp/typst")),
        },
        &mut selected,
        &mut fallback,
    );
    assert!(recompile_ok);
    assert_eq!(selected, "0.14.2");
    assert!(fallback.is_none());
    assert!(mgr.last_error().is_none());

    let (_notice_err, recompile_err) = mgr.apply_download_event(
        EngineDownloadEvent {
            version: "0.13.1".to_string(),
            result: Err("SHA mismatch".to_string()),
        },
        &mut selected,
        &mut fallback,
    );
    assert!(!recompile_err);
    assert_eq!(selected, "0.14.2");
    assert_eq!(mgr.last_error().as_deref(), Some("SHA mismatch"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_ensure_tls_crypto_provider_allows_reqwest_client_build() {
    super::ensure_tls_crypto_provider();
    let client = reqwest::Client::builder()
        .user_agent("azo-type-test/0.3.0")
        .build();
    assert!(
        client.is_ok(),
        "reqwest Client must build without panicking: {client:?}"
    );
}

#[test]
fn test_download_progress_fraction_and_eta_math() {
    use super::{DownloadProgress, DownloadStage};
    use std::time::{Duration, Instant};

    let prog = DownloadProgress {
        version: "0.14.2".to_string(),
        stage: DownloadStage::Downloading,
        downloaded_bytes: 5_000_000,
        total_bytes: Some(20_000_000),
        started_at: Instant::now() - Duration::from_secs(2),
    };
    let frac = prog.fraction().expect("fraction should exist");
    assert!((frac - 0.25).abs() < 0.001);
    assert!(prog.speed_bytes_per_sec() > 1_000_000.0);
    let eta = prog.eta_secs().expect("eta should exist");
    assert!(eta > 4.0 && eta < 8.0, "unexpected eta={eta}");
}

#[test]
#[ignore = "Requires live internet access to GitHub Releases"]
fn test_live_download_and_compile_0_14_2() {
    use std::collections::HashMap;
    use std::sync::Mutex;

    let dir = std::env::temp_dir().join(format!("azo-live-dl-{}", uuid::Uuid::new_v4()));
    let resources = Arc::new(SharedWorldResources::new());
    let mgr = EngineManager::with_paths(resources, dir.clone(), None);

    let asset = find_host_asset("0.14.2").expect("host asset 0.14.2 exists");
    let progress_map = Arc::new(Mutex::new(HashMap::new()));
    let res = super::archive::download_and_install_blocking(
        "0.14.2",
        asset.download_url,
        asset.sha256_hex,
        asset.archive_kind,
        asset.binary_name,
        &dir.join("0.14.2"),
        &progress_map,
        &egui::Context::default(),
    );
    eprintln!("download_and_install_blocking result: {res:?}");
    let bin_path = res.expect("live download should succeed");
    assert!(bin_path.is_file());

    let (engine, notice) = mgr.resolve_engine("0.14.2");
    assert!(notice.is_none());
    let mut project = crate::project::DiskProject::blank();
    if let Some(buf) = project
        .buffers
        .get_mut(&std::path::PathBuf::from("main.typ"))
    {
        buf.text = Some("= Hello 0.14.2\nLegacy compiler test.".to_string());
        buf.dirty = true;
    }
    let snap = engine.compile_project(&project, 1.5);
    eprintln!(
        "0.14.2 compile compiled_ok={} pages={} diags={:?} outline={:?}",
        snap.compiled_ok,
        snap.pages.len(),
        snap.diagnostics,
        snap.outline
    );
    assert!(snap.compiled_ok);
    assert_eq!(snap.pages.len(), 1);

    let pdf = engine
        .export_pdf_bytes(&project, &crate::core::pdf::PdfExportConfig::default())
        .expect("0.14.2 pdf export should succeed");
    assert!(pdf.starts_with(b"%PDF-"));

    let _ = fs::remove_dir_all(&dir);
}
