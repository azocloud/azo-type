//! Unit tests for global keyboard shortcut polling across editor and collab modes.

use egui::{Context, Event, Key, Modifiers, RawInput};

use super::*;

fn press_key(
    ctx: &Context,
    modifiers: Modifiers,
    key: Key,
    collab_active: bool,
) -> KeyboardShortcutOutcome {
    let mut input = RawInput {
        modifiers,
        ..Default::default()
    };
    input.events.push(Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    });
    ctx.begin_pass(input);
    let outcome = poll_keyboard_shortcuts(ctx, collab_active);
    let _ = ctx.end_pass();
    outcome
}

#[test]
fn polls_all_global_menu_shortcuts() {
    let ctx = Context::default();
    let cases = [
        (Modifiers::COMMAND, Key::Comma, MenuAction::OpenSettings),
        (Modifiers::COMMAND, Key::S, MenuAction::SaveFile),
        (Modifiers::COMMAND, Key::O, MenuAction::OpenFolder),
        (Modifiers::COMMAND, Key::N, MenuAction::NewFile),
        (Modifiers::COMMAND, Key::B, MenuAction::ToggleSidebar),
        (Modifiers::COMMAND, Key::J, MenuAction::ToggleBottomPanel),
        (Modifiers::COMMAND, Key::E, MenuAction::ExportPdf),
        (Modifiers::NONE, Key::F5, MenuAction::RescanFolder),
    ];

    for (mods, key, expected) in cases {
        let out = press_key(&ctx, mods, key, false);
        assert_eq!(out.menu_action, Some(expected), "failed for key {key:?}");
        assert!(!out.collab_undo);
        assert!(!out.collab_redo);
    }
}

#[test]
fn polls_collab_undo_and_redo_only_when_collab_is_active() {
    let ctx = Context::default();

    let inactive_undo = press_key(&ctx, Modifiers::COMMAND, Key::Z, false);
    assert!(!inactive_undo.collab_undo);
    assert!(!inactive_undo.collab_redo);
    assert_eq!(inactive_undo.menu_action, None);

    let active_undo = press_key(&ctx, Modifiers::COMMAND, Key::Z, true);
    assert!(active_undo.collab_undo);
    assert!(!active_undo.collab_redo);

    let active_redo_shift_z = press_key(&ctx, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z, true);
    assert!(active_redo_shift_z.collab_redo);
    assert!(!active_redo_shift_z.collab_undo);

    let active_redo_y = press_key(&ctx, Modifiers::COMMAND, Key::Y, true);
    assert!(active_redo_y.collab_redo);
    assert!(!active_redo_y.collab_undo);
}
