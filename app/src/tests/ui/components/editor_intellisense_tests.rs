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
    hover_hide_delay_ms: u64,
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
            hover_hide_delay_ms: 400,
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
            hover_hide_delay_ms: host.hover_hide_delay_ms,
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

#[test]
fn completion_popup_height_is_compact_and_anchored_near_caret_on_900px_viewport() {
    let mut doc = String::from("= Section One <sec-one>\n= Section Two <sec-two>\n");
    for i in 0..22 {
        doc.push_str(&format!("#let cand_{i:02} = {i}\n"));
    }
    doc.push_str("See ");

    let host = TestEditorHost::new(&doc);
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1000.0, 900.0))
        .build_ui_state(render_test_editor, host);
    focus_editor_text_edit(&mut harness);

    // Small list (2 items): `@sec-`.
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("@sec-".into()));
    harness.run();
    harness.run();

    let popup_id = Id::new("editor_intellisense_completion_popup");
    let small_rect = AreaState::load(&harness.ctx, popup_id)
        .map(|s| s.rect())
        .expect("small completion popup rect");
    assert!(
        small_rect.height() <= 280.0,
        "Expected compact small popup height <= 280px on 900px viewport, got {small_rect:?}"
    );

    // Accept or clear `@sec-`, then open a 22-item list (`#cand_`) near the bottom of the 900px viewport.
    harness.press_key(Key::Escape);
    harness.run();
    let filler_lines = "\n".repeat(42);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(format!("{filler_lines}#cand_")));
    harness.run();
    harness.run();

    let large_rect = AreaState::load(&harness.ctx, popup_id)
        .map(|s| s.rect())
        .expect("22-item completion popup rect");
    assert!(
        large_rect.height() <= 280.0,
        "Expected compact 22-item popup height <= 280px on 900px viewport, got {large_rect:?}"
    );
    assert!(
        large_rect.bottom() <= 900.0 && large_rect.right() <= 1000.0 && large_rect.left() >= 0.0,
        "Expected popup constrained near caret within viewport, got {large_rect:?}"
    );

    // Verify exact caret-adjacent anchoring both below (top of viewport) and above (bottom of viewport).
    struct CaretAnchorState {
        text: String,
        intel: EditorIntellisenseState,
        caret_screen: Option<egui::Rect>,
        move_end_once: bool,
    }
    let mut seed = String::new();
    for i in 0..20 {
        seed.push_str(&format!("#let cand_{i:02} = {i}\n"));
    }

    let run_caret_anchor_case = |initial_text: String| -> (egui::Rect, egui::Rect) {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1000.0, 900.0))
            .build_ui_state(
                |ui, st: &mut CaretAnchorState| {
                    let mut edit_out = egui::TextEdit::multiline(&mut st.text).show(ui);
                    if std::mem::take(&mut st.move_end_once) {
                        let end = CCursor::new(st.text.chars().count());
                        edit_out
                            .state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(end)));
                        edit_out.state.clone().store(ui.ctx(), edit_out.response.id);
                    }
                    let cursor_idx = edit_out
                        .state
                        .cursor
                        .char_range()
                        .map(|r| r.primary.index)
                        .unwrap_or(0);
                    let c = edit_out.galley.from_ccursor(CCursor::new(cursor_idx));
                    st.caret_screen = Some(
                        edit_out
                            .galley
                            .pos_from_cursor(&c)
                            .translate(edit_out.galley_pos.to_vec2()),
                    );
                    let mut pane_out = EditorPaneOutput::default();
                    let ctx_clone = ui.ctx().clone();
                    update_and_render_intellisense(
                        &ctx_clone,
                        ui,
                        IntellisenseUpdateParams {
                            active_file: Path::new("main.typ"),
                            compiler_diagnostics: &[],
                            dark_mode: true,
                            ime_in_progress: false,
                            accepted_completion: None,
                            hover_hide_delay_ms: 400,
                        },
                        &mut st.text,
                        &mut edit_out,
                        &mut st.intel,
                        &mut pane_out,
                    );
                },
                CaretAnchorState {
                    text: initial_text,
                    intel: EditorIntellisenseState::default(),
                    caret_screen: None,
                    move_end_once: true,
                },
            );
        h.get_by_role(egui::accesskit::Role::MultilineTextInput)
            .focus();
        h.state_mut().move_end_once = true;
        h.run();
        h.input_mut().events.push(egui::Event::Text("#cand".into()));
        h.run();
        h.run();
        assert!(
            h.state().intel.active_completion.is_some(),
            "expected active completion after typing #cand"
        );
        let caret = h.state().caret_screen.unwrap();
        let popup = AreaState::load(&h.ctx, popup_id).map(|s| s.rect()).unwrap();
        (caret, popup)
    };

    // 1) Caret near top -> placed directly below caret (`caret_screen.max.y + 4.0`).
    let (top_caret, top_popup) = run_caret_anchor_case(seed.clone());
    assert!(top_popup.height() <= 280.0);
    assert!(
        (top_popup.top() - (top_caret.max.y + 4.0)).abs() <= 4.0,
        "Expected popup anchored below caret ({top_caret:?}), got {top_popup:?}"
    );

    // 2) Caret near bottom of 900px viewport -> placed directly above caret (`caret_screen.min.y - 4.0`).
    let (bot_caret, bot_popup) = run_caret_anchor_case(format!("{seed}{}", "\n".repeat(38)));
    assert!(bot_popup.height() <= 280.0);
    assert!(
        (bot_popup.bottom() - (bot_caret.min.y - 4.0)).abs() <= 4.0,
        "Expected popup anchored above bottom caret ({bot_caret:?}), got {bot_popup:?}"
    );
}

#[test]
fn hover_popup_stays_continuously_open_without_flicker_and_has_bounded_height() {
    struct HoverStabilityState {
        text: String,
        where_pos: Option<Pos2>,
        figure_pos: Option<Pos2>,
    }

    let padding = "\n".repeat(22);
    let text = format!(
        "#show heading.where(level: 1): set text(blue){padding}#figure(rect(), caption: [Cap])\n"
    );
    let where_char = text.find("where").unwrap() + 2;
    let figure_char = text.find("figure").unwrap() + 2;

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 500.0))
        .build_ui_state(
            |ui, state: &mut HoverStabilityState| {
                let ctx_clone = ui.ctx().clone();
                let edit_output = egui::TextEdit::multiline(&mut state.text).show(ui);
                state.where_pos = Some(
                    edit_output
                        .galley
                        .pos_from_cursor(&edit_output.galley.from_ccursor(CCursor::new(where_char)))
                        .translate(edit_output.galley_pos.to_vec2())
                        .center(),
                );
                state.figure_pos = Some(
                    edit_output
                        .galley
                        .pos_from_cursor(
                            &edit_output.galley.from_ccursor(CCursor::new(figure_char)),
                        )
                        .translate(edit_output.galley_pos.to_vec2())
                        .center(),
                );

                if let Some(hover_char) = hovered_char_in_galley(&ctx_clone, ui, &edit_output) {
                    if let Some(hover) = compute_hover_info(&state.text, hover_char, &[], &[]) {
                        render_intellisense_hover(ui, &edit_output, &hover, true);
                    }
                }
            },
            HoverStabilityState {
                text,
                where_pos: None,
                figure_pos: None,
            },
        );

    harness.run();
    let where_target = harness.state().where_pos.expect("where target pos");
    let figure_target = harness.state().figure_pos.expect("figure target pos");
    let hover_area_id = Id::new("editor_intellisense_hover_popup");

    // Hovering `heading.where` near the top edge must remain continuously visible for 6 frames.
    for frame in 0..6 {
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(where_target));
        harness.run();
        assert!(
            harness.query_by_label("heading.where").is_some(),
            "hover popup flickered off on frame {frame} for heading.where"
        );
        let hr = AreaState::load(&harness.ctx, hover_area_id)
            .map(|s| s.rect())
            .expect("hover area state on heading.where");
        assert!(
            hr.height() <= 280.0,
            "Expected bounded hover height <= 280px on frame {frame}, got {hr:?}"
        );
        assert!(harness.query_by_label_contains("```example").is_none());
    }

    // Hovering `#figure` near the bottom edge with long docs must also stay open across 6 frames.
    for frame in 0..6 {
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(figure_target));
        harness.run();
        assert!(
            harness.query_by_label("figure").is_some(),
            "hover popup flickered off on frame {frame} for #figure"
        );
        let hr = AreaState::load(&harness.ctx, hover_area_id)
            .map(|s| s.rect())
            .expect("hover area state on #figure");
        assert!(
            hr.height() <= 280.0,
            "Expected bounded hover height <= 280px on frame {frame}, got {hr:?}"
        );
    }
}

struct GraceHoverHarnessState {
    text: String,
    intel: EditorIntellisenseState,
    hover_hide_delay_ms: u64,
    token_a_pos: Option<Pos2>,
    token_b_pos: Option<Pos2>,
    empty_pos: Option<Pos2>,
}

fn setup_grace_hover_harness(
    text: String,
    token_a_char: usize,
    token_b_char: usize,
    hover_hide_delay_ms: u64,
) -> Harness<'static, GraceHoverHarnessState> {
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 540.0))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(20)
        .build_ui_state(
            move |ui, st: &mut GraceHoverHarnessState| {
                let mut edit_out = egui::TextEdit::multiline(&mut st.text)
                    .desired_rows(20)
                    .show(ui);
                let char_center = |idx: usize| -> Pos2 {
                    edit_out
                        .galley
                        .pos_from_cursor(&edit_out.galley.from_ccursor(CCursor::new(idx)))
                        .translate(edit_out.galley_pos.to_vec2())
                        .center()
                };
                st.token_a_pos = Some(char_center(token_a_char));
                st.token_b_pos = Some(char_center(token_b_char));
                st.empty_pos = Some(Pos2::new(760.0, 500.0));

                let mut pane_out = EditorPaneOutput::default();
                let ctx_clone = ui.ctx().clone();
                update_and_render_intellisense(
                    &ctx_clone,
                    ui,
                    IntellisenseUpdateParams {
                        active_file: Path::new("main.typ"),
                        compiler_diagnostics: &[],
                        dark_mode: true,
                        ime_in_progress: false,
                        accepted_completion: None,
                        hover_hide_delay_ms: st.hover_hide_delay_ms,
                    },
                    &mut st.text,
                    &mut edit_out,
                    &mut st.intel,
                    &mut pane_out,
                );
            },
            GraceHoverHarnessState {
                text,
                intel: EditorIntellisenseState::default(),
                hover_hide_delay_ms,
                token_a_pos: None,
                token_b_pos: None,
                empty_pos: None,
            },
        );
    harness.run();
    harness
}

fn step_at(harness: &mut Harness<'_, GraceHoverHarnessState>, t: f64) {
    harness.input_mut().time = Some(t);
    harness.step();
    harness.input_mut().time = None;
}

#[test]
fn hover_popup_persists_during_grace_delay_and_hides_after_expiry() {
    let text = concat!(
        "#show heading.where(level: 1): set text(blue)\n\n",
        "#figure(rect(), caption: [Cap])\n",
    )
    .to_string();
    let where_char = text.find("where").unwrap() + 2;
    let figure_char = text.find("figure").unwrap() + 2;

    let mut harness = setup_grace_hover_harness(text, where_char, figure_char, 200);
    let where_pos = harness.state().token_a_pos.unwrap();
    let figure_pos = harness.state().token_b_pos.unwrap();
    let empty_pos = harness.state().empty_pos.unwrap();

    // Hover `heading.where` at t = 10.0s -> opens popup.
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(where_pos));
    step_at(&mut harness, 10.0);
    step_at(&mut harness, 10.02);
    assert!(harness.state().intel.active_hover.is_some());
    assert!(harness.query_by_label("heading.where").is_some());

    // Move pointer into empty space at t = 10.12s (+100ms < 200ms) -> stays alive!
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(empty_pos));
    step_at(&mut harness, 10.12);
    assert!(
        harness.state().intel.active_hover.is_some(),
        "Expected hover popup to stay alive during grace delay (+100ms < 200ms)"
    );
    assert!(harness.query_by_label("heading.where").is_some());

    // Advance to t = 10.25s (+230ms >= 200ms) -> hides!
    step_at(&mut harness, 10.25);
    assert!(
        harness.state().intel.active_hover.is_none(),
        "Expected hover popup to hide after grace delay expires (+230ms >= 200ms)"
    );
    assert!(harness.query_by_label("heading.where").is_none());

    // Passing over another token during the grace delay does not hijack the popup until expiry.
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(where_pos));
    step_at(&mut harness, 20.0);
    step_at(&mut harness, 20.02);
    assert_eq!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .map(|h| h.hover.title.as_str()),
        Some("heading.where")
    );

    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(figure_pos));
    step_at(&mut harness, 20.12);
    assert_eq!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .map(|h| h.hover.title.as_str()),
        Some("heading.where")
    );

    step_at(&mut harness, 20.25);
    assert_eq!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .map(|h| h.hover.title.as_str()),
        Some("figure")
    );
}

#[test]
fn hover_popup_stays_open_and_scrolls_when_pointer_enters_then_closes_on_exit() {
    let mut doc = String::from("#long_doc_fn(42)\n\n");
    for line_idx in 1..=28 {
        doc.push_str(&format!(
            "/// Doc line {line_idx:02}: detailed parameter explanation for scrolling.\n"
        ));
    }
    doc.push_str("#let long_doc_fn(x) = x\n");
    let call_char = doc.find("long_doc_fn").unwrap() + 3;

    let mut harness = setup_grace_hover_harness(doc, call_char, call_char, 200);
    let call_pos = harness.state().token_a_pos.unwrap();
    let empty_pos = harness.state().empty_pos.unwrap();

    // Open hover popup on `#long_doc_fn`.
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(call_pos));
    step_at(&mut harness, 10.0);
    step_at(&mut harness, 10.02);

    let (initial_placement, popup_rect) = {
        let ah = harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .expect("hover popup open");
        assert!(!ah.entered_popup);
        (ah.locked_placement, ah.last_rect.expect("popup last_rect"))
    };
    assert!(initial_placement.is_some());
    assert!(harness.query_by_label_contains("Doc line 01").is_some());

    // Moving pointer halfway toward the popup keeps placement locked (no jumping/running away).
    let halfway_pos = Pos2::new(
        (call_pos.x + popup_rect.center().x) * 0.5,
        (call_pos.y + popup_rect.center().y) * 0.5,
    );
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(halfway_pos));
    step_at(&mut harness, 10.06);
    assert_eq!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .and_then(|ah| ah.locked_placement),
        initial_placement,
        "Hover popup placement must stay locked while pointer moves toward it"
    );

    // Move pointer INSIDE the popup and hold well beyond hover_hide_delay_ms (200ms -> +800ms).
    let inside_popup = popup_rect.center();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(inside_popup));
    step_at(&mut harness, 10.10);
    assert!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .is_some_and(|ah| ah.entered_popup),
        "Expected entered_popup = true when pointer enters hover popup"
    );

    step_at(&mut harness, 10.90);
    assert!(
        harness.state().intel.active_hover.is_some(),
        "Expected hover popup to stay open indefinitely while pointer is inside it"
    );

    // Measure initial vertical position of Doc line 01, then scroll down inside the popup.
    let doc_y_before = harness
        .get_by_label_contains("Doc line 01")
        .bounding_box()
        .expect("Doc line 01 bounding box before scroll")
        .y0;
    harness.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::Vec2::new(0.0, -180.0),
        modifiers: Modifiers::NONE,
    });
    step_at(&mut harness, 11.0);
    harness.run_steps(4);
    assert!(
        harness
            .state()
            .intel
            .active_hover
            .as_ref()
            .is_some_and(|ah| ah.entered_popup),
        "Scrolling inside hover popup must not close it"
    );
    let doc_y_after = harness
        .get_by_label_contains("Doc line 01")
        .bounding_box()
        .expect("Doc line 01 bounding box after scroll")
        .y0;
    assert!(
        doc_y_after < doc_y_before - 5.0,
        "Expected documentation inside hover popup to scroll upward (before={doc_y_before}, after={doc_y_after})"
    );

    // Moving pointer OUT of the popup into empty space closes it immediately.
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(empty_pos));
    step_at(&mut harness, 11.10);
    assert!(
        harness.state().intel.active_hover.is_none(),
        "Expected hover popup to close immediately when pointer exits popup into empty space"
    );
    assert!(harness.query_by_label_contains("Doc line 01").is_none());
}

#[test]
fn active_hover_clears_when_project_changes_or_completion_opens() {
    let mut state = EditorIntellisenseState {
        active_hover: Some(ActiveHoverPopup {
            hover: crate::core::intellisense::HoverInfo {
                start_char: 0,
                end_char: 4,
                title: "test".into(),
                detail: String::new(),
                documentation: "doc".into(),
                is_diagnostic: false,
            },
            last_token_hover_time: 1.0,
            entered_popup: false,
            locked_placement: None,
            last_rect: None,
        }),
        ..Default::default()
    };

    let mut project = DiskProject::blank();
    state.refresh_project_cache(&project);
    assert!(
        state.active_hover.is_none(),
        "Changing project revision/active file must clear active_hover"
    );

    // Editing text inside the editor clears active_hover immediately.
    let text = "#show heading.where(level: 1): set text(blue)\n".to_string();
    let where_char = text.find("where").unwrap() + 2;
    let mut harness = setup_grace_hover_harness(text, where_char, where_char, 400);
    let where_pos = harness.state().token_a_pos.unwrap();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(where_pos));
    step_at(&mut harness, 10.0);
    step_at(&mut harness, 10.02);
    assert!(harness.state().intel.active_hover.is_some());

    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(" #".into()));
    step_at(&mut harness, 10.05);
    assert!(
        harness.state().intel.active_hover.is_none(),
        "Editing text / opening completion must clear active_hover"
    );
    project.revision += 1;
    let _ = project;
}
