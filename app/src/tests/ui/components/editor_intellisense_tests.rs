//! Unit and headless `egui_kittest` tests for Typst IntelliSense completion, hover, and diagnostic UI.

use std::fs;
use std::path::{Path, PathBuf};

use egui::text::CCursor;
use egui::{AreaState, Id, Key, Modifiers, Pos2};
use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;
use crate::core::intellisense::{CompletionItem, CompletionKind, CompletionRequestContext};
use crate::core::spellcheck::{DiagnosticCategory, LanguageCheckDiagnostic};
use crate::project::buffer::OpenBuffer;
use crate::project::DiskProject;
use crate::ui::components::editor::{render_editor_pane, EditorPaneParams};
use crate::ui::components::editor_diagnostics::paint_language_diagnostics;

struct TestEditorHost {
    project: DiskProject,
    font_size: f32,
    word_wrap: bool,
    inspect_asset_mode: bool,
    clipboard_fallback: String,
    intellisense: EditorIntellisenseState,
    compiler_diagnostics: Vec<DiagnosticItem>,
    last_text_changed: bool,
    focus_editor_once: bool,
    move_cursor_to_end_once: bool,
}

impl TestEditorHost {
    fn new(initial_text: &str) -> Self {
        let mut project = DiskProject::blank();
        if let Some(buf) = project.active_buffer_mut() {
            buf.text = Some(initial_text.to_string());
            buf.size_bytes = initial_text.len() as u64;
        }
        Self {
            project,
            font_size: 14.0,
            word_wrap: true,
            inspect_asset_mode: false,
            clipboard_fallback: String::new(),
            intellisense: EditorIntellisenseState::default(),
            compiler_diagnostics: Vec::new(),
            last_text_changed: false,
            focus_editor_once: true,
            move_cursor_to_end_once: true,
        }
    }

    fn active_text(&self) -> &str {
        self.project
            .active_buffer()
            .and_then(|b| b.text.as_deref())
            .unwrap_or("")
    }
}

fn render_test_editor(ui: &mut Ui, host: &mut TestEditorHost) {
    let ctx = ui.ctx().clone();
    let active_path = host.project.active_file.clone();
    let end_char = host.active_text().chars().count();
    let pending_cursor_restore = if std::mem::take(&mut host.move_cursor_to_end_once) {
        Some(CharSelection {
            primary_char: end_char,
            secondary_char: end_char,
        })
    } else {
        None
    };

    let out = render_editor_pane(
        &ctx,
        ui,
        EditorPaneParams {
            dark_mode: true,
            editor_font_size: &mut host.font_size,
            word_wrap: &mut host.word_wrap,
            show_line_numbers: true,
            syntax_highlighting: true,
            inspect_asset_mode: &mut host.inspect_asset_mode,
            clipboard_fallback: &mut host.clipboard_fallback,
            project: &mut host.project,
            pending_cursor_restore,
            pending_scroll_to_char: None,
            pending_focus_editor: false,
            search_highlight_range: None,
            last_editor_selection: None,
            pending_editor_context_action: None,
            peers_in_active_file: &[],
            language_diagnostics: &[],
            compiler_diagnostics: &host.compiler_diagnostics,
            intellisense: &mut host.intellisense,
        },
    );
    host.last_text_changed |= out.text_changed;

    if std::mem::take(&mut host.focus_editor_once) {
        let editor_id = ui.make_persistent_id(Id::new(("typst_code_editor", &active_path)));
        // Find focusable TextEdit in memory by requesting focus on the known salt/id if needed.
        let _ = editor_id;
    }
}

fn focus_editor_text_edit(harness: &mut Harness<'_, TestEditorHost>) {
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness.state_mut().move_cursor_to_end_once = true;
    harness.run();
}

#[test]
fn lazy_project_cache_loads_open_and_unopened_disk_sources_once_per_revision() {
    let root = std::env::temp_dir().join(format!("azo_isense_cache_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("chapters")).unwrap();
    fs::write(root.join("main.typ"), "= Main\n").unwrap();
    fs::write(
        root.join("chapters/methods.typ"),
        "== Methods <sec-methods>\n",
    )
    .unwrap();
    fs::write(root.join("refs.bib"), "@article{knuth84, title={TeX}}\n").unwrap();
    fs::write(root.join("diagram.png"), b"png").unwrap();

    let mut project = DiskProject::open_folder(&root, false).expect("open disk project");
    let mut state = EditorIntellisenseState::default();
    state.refresh_project_cache(&project);

    assert!(state
        .project_files()
        .iter()
        .any(|p| p == Path::new("diagram.png")));
    assert!(state
        .cached_sources()
        .iter()
        .any(|(p, s)| p == Path::new("chapters/methods.typ") && s.contains("sec-methods")));
    assert!(state
        .cached_sources()
        .iter()
        .any(|(p, s)| p == Path::new("refs.bib") && s.contains("knuth84")));

    // Modifying an unopened disk file without changing revision does not re-read on every frame.
    fs::write(
        root.join("refs.bib"),
        "@article{lamport94, title={LaTeX}}\n",
    )
    .unwrap();
    state.refresh_project_cache(&project);
    assert!(state
        .cached_sources()
        .iter()
        .any(|(p, s)| p == Path::new("refs.bib") && s.contains("knuth84")));

    // Bumping revision refreshes from disk.
    project.revision += 1;
    state.refresh_project_cache(&project);
    assert!(state
        .cached_sources()
        .iter()
        .any(|(p, s)| p == Path::new("refs.bib") && s.contains("lamport94")));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn pre_text_edit_keys_navigate_accept_dismiss_and_ignore_modified_shortcuts() {
    let ctx = Context::default();
    let sample_items = vec![
        CompletionItem {
            label: "tab-alpha".into(),
            insert_text: "tab-alpha".into(),
            detail: "Figure".into(),
            documentation: "Alpha table".into(),
            kind: CompletionKind::Reference,
            source_hint: Some("main.typ:3".into()),
        },
        CompletionItem {
            label: "tab-beta".into(),
            insert_text: "tab-beta".into(),
            detail: "Figure".into(),
            documentation: "Beta table".into(),
            kind: CompletionKind::Reference,
            source_hint: Some("main.typ:8".into()),
        },
    ];

    let mut state = EditorIntellisenseState {
        active_completion: Some(CompletionRequestContext {
            replace_start_char: 5,
            replace_end_char: 9,
            prefix: "tab-".into(),
            items: sample_items.clone(),
        }),
        ..Default::default()
    };

    // Unfocused editor never consumes keys.
    ctx.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    });
    assert_eq!(
        handle_intellisense_keys_before_text_edit(&ctx, false, &mut state),
        (false, None)
    );
    let _ = ctx.end_pass();

    // Modified shortcuts like Alt+ArrowUp must not be consumed by IntelliSense.
    ctx.begin_pass(egui::RawInput {
        modifiers: Modifiers::ALT,
        events: vec![egui::Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::ALT,
        }],
        ..Default::default()
    });
    assert_eq!(
        handle_intellisense_keys_before_text_edit(&ctx, true, &mut state),
        (false, None)
    );
    let _ = ctx.end_pass();

    // ArrowDown wraps from 0 -> 1, ArrowUp wraps 0 -> 1, Tab accepts second item.
    ctx.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    });
    assert_eq!(
        handle_intellisense_keys_before_text_edit(&ctx, true, &mut state),
        (true, None)
    );
    assert_eq!(state.selected_index, 1);
    let _ = ctx.end_pass();

    ctx.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    });
    let (consumed, accepted) = handle_intellisense_keys_before_text_edit(&ctx, true, &mut state);
    assert!(consumed);
    assert_eq!(
        accepted,
        Some(AcceptedCompletion {
            replace_start_char: 5,
            replace_end_char: 9,
            insert_text: "tab-beta".into(),
        })
    );
    assert!(state.active_completion.is_none());
    let _ = ctx.end_pass();

    // Escape dismisses and records cursor position.
    state.active_completion = Some(CompletionRequestContext {
        replace_start_char: 5,
        replace_end_char: 9,
        prefix: "tab-".into(),
        items: sample_items,
    });
    ctx.begin_pass(egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    });
    assert_eq!(
        handle_intellisense_keys_before_text_edit(&ctx, true, &mut state),
        (true, None)
    );
    assert!(state.active_completion.is_none());
    assert_eq!(state.dismissed_at_cursor, Some(9));
    let _ = ctx.end_pass();
}

#[test]
fn map_diagnostics_to_char_spans_filters_file_and_maps_line_columns() {
    let text = "first line\n#let x = unknown\nthird";
    let diags = vec![
        DiagnosticItem {
            is_error: true,
            file: PathBuf::from("./main.typ"),
            line: Some(2),
            column: Some(10),
            message: "unknown variable: unknown".into(),
            hints: vec!["check spelling".into()],
        },
        DiagnosticItem {
            is_error: false,
            file: PathBuf::from("other.typ"),
            line: Some(1),
            column: Some(1),
            message: "ignored".into(),
            hints: vec![],
        },
    ];

    let mapped = map_diagnostics_to_char_spans(text, Path::new("main.typ"), &diags);
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].0, 20);
    assert_eq!(mapped[0].1, 27);
    assert!(mapped[0].2.contains("unknown variable: unknown"));
    assert!(mapped[0].2.contains("check spelling"));
    assert!(mapped[0].3);
}

#[test]
fn at_ref_and_macro_completion_popups_show_docs_and_accept_via_tab_enter_and_click() {
    let doc = concat!(
        "/// Renders a highlighted metric badge.\n",
        "#let badge_pill(label) = [*#label*]\n\n",
        "#figure(table([A]), caption: [Latency Benchmark]) <tab-results>\n\n",
        "See ",
    );
    let mut host = TestEditorHost::new(doc);
    host.project.buffers.insert(
        PathBuf::from("refs.bib"),
        OpenBuffer {
            rel_path: PathBuf::from("refs.bib"),
            abs_path: PathBuf::new(),
            kind: FileKind::Bib,
            text: Some("@article{knuth84,\n  title={The TeXbook}\n}\n".to_string()),
            size_bytes: 40,
            dirty: false,
        },
    );
    host.project.revision += 1;

    let mut harness = Harness::new_ui_state(render_test_editor, host);
    focus_editor_text_edit(&mut harness);

    // Move cursor to the end after "See " and type "@tab-".
    let end_idx = harness.state().active_text().chars().count();
    harness.state_mut().project.active_buffer_mut();
    let _ = end_idx;
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("@tab-".into()));
    harness.run();

    // Completion popup should show @tab-results with surrounding caption context & footer.
    let header_text = t!("intellisense.suggestions_header", locale = "en").to_string();
    let accept_hint = t!("intellisense.hint_accept", locale = "en").to_string();
    assert!(harness.query_by_label(&header_text).is_some());
    assert!(harness.query_all_by_label("tab-results").next().is_some());
    assert!(harness
        .query_by_label_contains("Latency Benchmark")
        .is_some());
    assert!(harness.query_by_label(&accept_hint).is_some());

    // Pressing Tab accepts completion without inserting a literal '\t'.
    harness.press_key(Key::Tab);
    harness.run();
    assert!(harness.state().active_text().ends_with("See @tab-results"));
    assert!(!harness.state().active_text().contains('\t'));
    assert!(harness.state().last_text_changed);
    assert!(harness.query_by_label(&header_text).is_none());

    // Typing " #badge_" triggers user #let macro completion with doc comment, Enter inserts it cleanly.
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(" #badge_".into()));
    harness.run();
    assert!(harness.query_all_by_label("badge_pill").next().is_some());
    assert!(harness
        .query_by_label_contains("Renders a highlighted metric badge.")
        .is_some());
    harness.press_key(Key::Enter);
    harness.run();
    assert!(harness
        .state()
        .active_text()
        .ends_with("See @tab-results #badge_pill"));

    // Typing " @knu" opens BibTeX citation completion; clicking the candidate accepts it.
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(" @knu".into()));
    harness.run();
    let knuth_btn = harness
        .get_all_by_label("knuth84")
        .next()
        .expect("knuth84 candidate button");
    knuth_btn.click();
    harness.run();
    assert!(harness
        .state()
        .active_text()
        .ends_with("See @tab-results #badge_pill @knuth84"));
    assert!(harness.query_by_label(&header_text).is_none());

    // Typing " @tab" and pressing Esc closes popup and keeps it dismissed at the same cursor.
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(" @tab".into()));
    harness.run();
    assert!(harness.query_all_by_label("tab-results").next().is_some());
    harness.press_key(Key::Escape);
    harness.run();
    assert!(harness.query_by_label(&header_text).is_none());
    assert!(harness.state().intellisense.dismissed_at_cursor.is_some());
}

#[test]
fn spell_diagnostic_tooltip_and_intellisense_hover_coexist_simultaneously() {
    struct CoexistState {
        text: String,
        hover_point: Option<Pos2>,
    }

    let text = "Use #heading[Title] here".to_string();
    // Span for "heading": chars 5..12.
    let lang_diag = LanguageCheckDiagnostic {
        category: DiagnosticCategory::Spelling,
        start_byte: 5,
        end_byte: 12,
        start_char: 5,
        end_char: 12,
        line: 1,
        column: 6,
        excerpt: "heading".into(),
        message: "Spellcheck note on heading".into(),
        rule_id: "SPELL/TEST".into(),
        suggestions: vec!["Heading".into()],
    };

    let mut harness = Harness::new_ui_state(
        |ui, state: &mut CoexistState| {
            let ctx_clone = ui.ctx().clone();
            let edit_output = egui::TextEdit::multiline(&mut state.text).show(ui);
            let target_pos = edit_output
                .galley
                .pos_from_cursor(&edit_output.galley.from_ccursor(CCursor::new(7)))
                .translate(edit_output.galley_pos.to_vec2())
                .center();
            state.hover_point = Some(target_pos);

            paint_language_diagnostics(
                ui,
                &edit_output,
                std::slice::from_ref(&lang_diag),
                true,
                state.text.chars().count(),
            );

            if let Some(hover_char) = hovered_char_in_galley(&ctx_clone, ui, &edit_output) {
                if let Some(hover) = compute_hover_info(&state.text, hover_char, &[], &[]) {
                    render_intellisense_hover(ui, &edit_output, &hover, true);
                }
            }
        },
        CoexistState {
            text,
            hover_point: None,
        },
    );

    // First frame computes target_pos over "heading".
    harness.run();
    let target = harness.state().hover_point.expect("hover target computed");

    // Move pointer onto "heading" and run two frames so both popups settle.
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(target));
    harness.run();
    harness.run();

    // BOTH spell/grammar diagnostic tooltip AND IntelliSense hover popup texts must be visible!
    assert!(harness
        .query_by_label_contains("Spellcheck note on heading")
        .is_some());
    assert!(harness.query_by_label("heading").is_some());
    assert!(harness.query_by_label_contains("section heading").is_some());

    // Verify their popup area rectangles do not overlap.
    let diag_area_id = egui::tooltip_id(Id::new("editor_language_diag_tooltip"), 0);
    let hover_area_id = Id::new("editor_intellisense_hover_popup");
    let (diag_rect, hover_rect) = harness.ctx.memory(|_| {
        let dr = AreaState::load(&harness.ctx, diag_area_id).map(|s| s.rect());
        let hr = AreaState::load(&harness.ctx, hover_area_id).map(|s| s.rect());
        (dr, hr)
    });
    let dr = diag_rect.expect("diagnostic tooltip rect exists");
    let hr = hover_rect.expect("intellisense hover rect exists");
    assert!(
        !dr.intersects(hr),
        "Expected non-overlapping popups, got diag={dr:?}, hover={hr:?}"
    );
}
