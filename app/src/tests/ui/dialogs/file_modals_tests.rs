//! Pure unit and headless `egui_kittest` tests for New File and Rename File modals.

use std::path::PathBuf;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;

struct ModalHarnessState {
    buf: String,
    confirmed: bool,
    cancelled: bool,
    old_rel: PathBuf,
}

#[test]
fn try_confirm_new_file_rejects_blank_and_trims_valid_name() {
    let mut open = true;
    assert_eq!(try_confirm_new_file("   \t ", &mut open), None);
    assert!(open);

    let result = try_confirm_new_file("  notes.typ  ", &mut open);
    assert_eq!(result.as_deref(), Some("notes.typ"));
    assert!(!open);
}

#[test]
fn try_confirm_rename_file_requires_non_empty_name_and_clears_target() {
    let mut target = Some(PathBuf::from("old.typ"));
    assert_eq!(try_confirm_rename_file(&mut target, "  "), None);
    assert!(target.is_some());

    let renamed = try_confirm_rename_file(&mut target, "  new.typ ");
    assert_eq!(
        renamed,
        Some((PathBuf::from("old.typ"), "new.typ".to_string()))
    );
    assert!(target.is_none());
}

#[test]
fn closed_file_modals_return_none() {
    let ctx = egui::Context::default();
    let mut open = false;
    let mut buf = "foo.typ".to_string();
    assert_eq!(render_new_file_modal(&ctx, &mut open, &mut buf, true), None);

    let mut target = None;
    assert_eq!(
        render_rename_file_modal(&ctx, &mut target, &mut buf, false),
        None
    );
}

#[test]
fn new_file_body_renders_and_triggers_create_or_cancel() {
    let state = ModalHarnessState {
        buf: "chapter1.typ".to_string(),
        confirmed: false,
        cancelled: false,
        old_rel: PathBuf::new(),
    };

    let create_label = t!("file_modals.create_btn", locale = "en").to_string();
    let cancel_label = t!("file_modals.cancel_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut ModalHarnessState| {
            let (cr, cl) = render_new_file_body(ui, &mut state.buf);
            state.confirmed |= cr;
            state.cancelled |= cl;
        },
        state,
    );

    harness.get_by_label(&create_label).click();
    harness.run();
    assert!(harness.state().confirmed);
    assert!(!harness.state().cancelled);

    harness.get_by_label(&cancel_label).click();
    harness.run();
    assert!(harness.state().cancelled);
}

#[test]
fn rename_file_body_renders_prompt_and_triggers_rename_or_cancel() {
    let state = ModalHarnessState {
        buf: "final.typ".to_string(),
        confirmed: false,
        cancelled: false,
        old_rel: PathBuf::from("draft.typ"),
    };

    let rename_label = t!("file_modals.rename_btn", locale = "en").to_string();
    let cancel_label = t!("file_modals.cancel_btn", locale = "en").to_string();

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut ModalHarnessState| {
            let (rn, cl) = render_rename_file_body(ui, &state.old_rel, &mut state.buf);
            state.confirmed |= rn;
            state.cancelled |= cl;
        },
        state,
    );

    harness.get_by_label(&rename_label).click();
    harness.run();
    assert!(harness.state().confirmed);

    harness.get_by_label(&cancel_label).click();
    harness.run();
    assert!(harness.state().cancelled);
}
