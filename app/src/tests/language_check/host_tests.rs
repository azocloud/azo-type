//! Unit tests for `LanguageCheckHost` background checking, debouncing, and poll transitions.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Context, RawInput};

use super::*;

#[test]
fn check_now_clears_results_when_disabled_or_non_typst() {
    let mut host = LanguageCheckHost::new(None);
    let mut settings = IdeSettings::default();
    let mut project = DiskProject::blank();

    host.checking = true;
    host.missing_tier1_pack = Some("en".to_string());
    host.last_warning = Some("warn".to_string());

    // 1. Disabled in settings clears state immediately.
    settings.spell_check_enabled = false;
    host.check_now(&settings, &project, "en");
    assert!(!host.checking);
    assert!(host.missing_tier1_pack.is_none());
    assert!(host.last_warning.is_none());

    // 2. Enabled, but active file is non-Typst -> clears results.
    settings.spell_check_enabled = true;
    project.create_file_on_disk("notes.md").unwrap();
    host.checking = true;
    host.check_now(&settings, &project, "en");
    assert!(!host.checking);
}

#[test]
fn check_now_and_poll_completes_worker_roundtrip_on_typst_buffer() {
    let mut host = LanguageCheckHost::new(None);
    let settings = IdeSettings {
        spell_check_enabled: true,
        grammar_check_enabled: false,
        spell_check_language: "en".to_string(),
        ..Default::default()
    };

    let mut project = DiskProject::blank();
    if let Some(buf) = project.active_buffer_mut() {
        buf.text = Some("= Heading\nHello the the world.\n".to_string());
    }

    let ctx = Context::default();
    host.was_enabled = true;
    host.check_now(&settings, &project, "en");
    assert!(host.checking);

    let deadline = Instant::now() + Duration::from_secs(10);
    while host.checking && Instant::now() < deadline {
        let _ = host.poll(&ctx, &settings, &project, "en");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!host.checking, "worker should complete within timeout");
    assert_eq!(host.checked_file, PathBuf::from("main.typ"));
}

#[test]
fn poll_handles_enable_disable_debounce_and_download_outcomes() {
    let mut host = LanguageCheckHost::new(None);
    let mut settings = IdeSettings::default();
    let project = DiskProject::blank();
    let ctx = Context::default();

    // Master enable transition triggers check_now.
    settings.spell_check_enabled = true;
    settings.auto_spell_check = true;
    settings.spell_check_debounce_ms = 50;
    let _ = host.poll(&ctx, &settings, &project, "en");
    assert!(host.was_enabled);

    // Edit at t=1.0 schedules pending check.
    let input = RawInput {
        time: Some(1.0),
        ..Default::default()
    };
    ctx.begin_pass(input);
    host.on_text_edited(&ctx, &settings);
    let _ = ctx.end_pass();
    assert!(host.pending_check);

    // Poll before debounce elapsed keeps pending_check true.
    let input2 = RawInput {
        time: Some(1.01),
        ..Default::default()
    };
    ctx.begin_pass(input2);
    let _ = host.poll(&ctx, &settings, &project, "en");
    let _ = ctx.end_pass();
    assert!(host.pending_check);

    // Poll after debounce elapsed fires check_now and clears pending_check.
    let input3 = RawInput {
        time: Some(1.2),
        ..Default::default()
    };
    ctx.begin_pass(input3);
    let _ = host.poll(&ctx, &settings, &project, "en");
    let _ = ctx.end_pass();
    assert!(!host.pending_check);

    // Simulate download outcomes over internal channel.
    host.dl_tx
        .send(DictionaryDownloadOutcome {
            lang_code: "en".to_string(),
            result: Ok(()),
        })
        .unwrap();
    let ok_notice = host.poll(&ctx, &settings, &project, "en");
    assert!(ok_notice.unwrap().contains("Downloaded language pack 'en'"));

    host.dl_tx
        .send(DictionaryDownloadOutcome {
            lang_code: "de".to_string(),
            result: Err("network offline".to_string()),
        })
        .unwrap();
    let err_notice = host.poll(&ctx, &settings, &project, "en");
    assert!(err_notice.unwrap().contains("network offline"));

    // Master disable transition clears results.
    settings.spell_check_enabled = false;
    let _ = host.poll(&ctx, &settings, &project, "en");
    assert!(!host.was_enabled);
    assert!(!host.checking);
}
