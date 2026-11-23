//! Keyboard shortcut polling for desktop editor, panels, and collaboration undo/redo.

use egui::{Context, Key, Modifiers};

use crate::platform::menu::MenuAction;

/// Outcome of polling global application keyboard shortcuts for the current frame.
#[derive(Debug, Default)]
pub struct KeyboardShortcutOutcome {
    pub menu_action: Option<MenuAction>,
    pub collab_undo: bool,
    pub collab_redo: bool,
}

/// Polls and consumes active global keyboard shortcuts on `ctx`.
pub fn poll_keyboard_shortcuts(ctx: &Context, collab_active: bool) -> KeyboardShortcutOutcome {
    let mut outcome = KeyboardShortcutOutcome::default();

    ctx.input_mut(|i| {
        let shortcuts = [
            (Modifiers::COMMAND, Key::Comma, MenuAction::OpenSettings),
            (Modifiers::COMMAND, Key::S, MenuAction::SaveFile),
            (Modifiers::COMMAND, Key::O, MenuAction::OpenFolder),
            (Modifiers::COMMAND, Key::N, MenuAction::NewFile),
            (Modifiers::COMMAND, Key::F, MenuAction::Find),
            (Modifiers::COMMAND, Key::H, MenuAction::FindAndReplace),
            (Modifiers::COMMAND, Key::B, MenuAction::ToggleSidebar),
            (Modifiers::COMMAND, Key::J, MenuAction::ToggleBottomPanel),
            (Modifiers::COMMAND, Key::E, MenuAction::ExportPdf),
            (Modifiers::NONE, Key::F5, MenuAction::RescanFolder),
        ];
        for (m, k, act) in shortcuts {
            if i.consume_key(m, k) {
                outcome.menu_action = Some(act);
                return;
            }
        }
        if collab_active {
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || i.consume_key(Modifiers::COMMAND, Key::Y)
            {
                outcome.collab_redo = true;
            } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
                outcome.collab_undo = true;
            }
        }
    });

    outcome
}

/// Polls and consumes editor-scoped keyboard shortcuts on `ctx`.
///
/// Callers should invoke this only while the source code editor has focus so these bindings
/// do not hijack inputs in search fields or modal dialogs.
pub fn poll_editor_shortcuts(ctx: &Context) -> Option<MenuAction> {
    ctx.input_mut(|i| {
        let shortcuts = [
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Slash,
                MenuAction::ToggleBlockComment,
            ),
            (
                Modifiers::COMMAND,
                Key::Slash,
                MenuAction::ToggleLineComment,
            ),
            (
                Modifiers::COMMAND,
                Key::CloseBracket,
                MenuAction::IndentLines,
            ),
            (
                Modifiers::COMMAND,
                Key::OpenBracket,
                MenuAction::OutdentLines,
            ),
            (Modifiers::COMMAND, Key::D, MenuAction::DuplicateLine),
            (Modifiers::ALT, Key::ArrowUp, MenuAction::MoveLineUp),
            (Modifiers::ALT, Key::ArrowDown, MenuAction::MoveLineDown),
        ];
        for (m, k, act) in shortcuts {
            if i.consume_key(m, k) {
                return Some(act);
            }
        }
        None
    })
}

#[cfg(test)]
#[path = "../tests/platform/shortcuts_tests.rs"]
mod tests;
