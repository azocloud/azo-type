//! Typst IntelliSense UI state, keyboard handling, completion popup, and editor integration.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::text::{CCursor, CCursorRange};
use egui::widgets::text_edit::TextEditOutput;
use egui::{
    Align, Align2, Area, AreaState, Context, Frame, Id, Key, Margin, Modifiers, Order, Pos2,
    RichText, ScrollArea, Sense, Stroke, Ui, UiKind, Vec2,
};
use rust_i18n::t;

use crate::core::diff::CharSelection;
use crate::core::file_kind::FileKind;
#[cfg(test)]
use crate::core::intellisense::compute_hover_info;
use crate::core::intellisense::{
    compute_completions, compute_typst_hints, CompletionRequestContext,
};
use crate::engine::DiagnosticItem;
use crate::project::{DiskProject, FsTreeNode};
use crate::ui::components::editor::EditorPaneOutput;
use crate::ui::components::editor_intellisense_hover::{
    completion_kind_badge, render_completion_doc_pane,
};
#[cfg(test)]
use crate::ui::components::editor_intellisense_hover::{
    hovered_char_in_galley, render_intellisense_hover,
};
pub use crate::ui::components::editor_intellisense_hover::{
    map_diagnostics_to_char_spans, paint_typst_diagnostic_hints,
    update_and_render_intellisense_hover, ActiveHoverPopup, HoverPopupParams,
};
use crate::ui::theme::ThemePalette;

#[cfg(test)]
#[path = "../../tests/ui/components/editor_intellisense_tests.rs"]
mod tests;

/// Accepted completion candidate ready to replace `[replace_start_char, replace_end_char)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedCompletion {
    pub replace_start_char: usize,
    pub replace_end_char: usize,
    pub insert_text: String,
}

/// Persistent editor IntelliSense state, completion selection, and lazy project source cache.
#[derive(Debug, Default)]
pub struct EditorIntellisenseState {
    pub active_completion: Option<CompletionRequestContext>,
    pub selected_index: usize,
    pub dismissed_at_cursor: Option<usize>,
    pub active_hover: Option<ActiveHoverPopup>,
    cached_revision: u64,
    cached_active_file: PathBuf,
    cached_buffer_count: usize,
    cached_sources: Vec<(PathBuf, String)>,
    cached_files: Vec<PathBuf>,
    last_cursor_char: Option<usize>,
    scroll_selection_into_view: bool,
    pub(crate) editor_had_focus: bool,
}

impl EditorIntellisenseState {
    /// Lazily refreshes cached project files and unopened `.typ`/`.bib` sources when the project
    /// revision, active file, or open buffer count changes.
    pub fn refresh_project_cache(&mut self, project: &DiskProject) {
        if self.cached_revision == project.revision
            && self.cached_active_file == project.active_file
            && self.cached_buffer_count == project.buffers.len()
        {
            return;
        }

        let mut files = Vec::new();
        collect_tree_files_recursive(&project.tree, &mut files);
        let mut file_set: HashSet<PathBuf> = files.iter().cloned().collect();
        for rel_path in project.buffers.keys() {
            if file_set.insert(rel_path.clone()) {
                files.push(rel_path.clone());
            }
        }

        let mut sources = Vec::new();
        let mut seen_sources = HashSet::new();

        for (rel_path, buf) in &project.buffers {
            if is_intellisense_source_path(rel_path, Some(buf.kind)) {
                if let Some(ref text) = buf.text {
                    seen_sources.insert(rel_path.clone());
                    sources.push((rel_path.clone(), text.clone()));
                }
            }
        }

        if project.has_disk_backing() {
            for rel_path in &files {
                if seen_sources.contains(rel_path) || !is_intellisense_source_path(rel_path, None) {
                    continue;
                }
                if let Ok(content) = project.read_source_text(rel_path) {
                    seen_sources.insert(rel_path.clone());
                    sources.push((rel_path.clone(), content));
                }
            }
        }

        self.cached_files = files;
        self.cached_sources = sources;
        self.cached_revision = project.revision;
        self.cached_active_file = project.active_file.clone();
        self.cached_buffer_count = project.buffers.len();
        self.active_hover = None;
    }

    /// Returns cached project-relative file paths for path completion.
    pub fn project_files(&self) -> &[PathBuf] {
        &self.cached_files
    }

    /// Returns cached `(path, content)` project source entries.
    pub fn cached_sources(&self) -> &[(PathBuf, String)] {
        &self.cached_sources
    }
}

fn build_source_refs<'a>(
    cached_sources: &'a [(PathBuf, String)],
    active_file: &'a Path,
    active_text: &'a str,
) -> Vec<(&'a Path, &'a str)> {
    let mut refs = Vec::with_capacity(cached_sources.len() + 1);
    refs.push((active_file, active_text));
    for (path, content) in cached_sources {
        if path != active_file {
            refs.push((path.as_path(), content.as_str()));
        }
    }
    refs
}

fn collect_tree_files_recursive(nodes: &[FsTreeNode], out: &mut Vec<PathBuf>) {
    for node in nodes {
        match node {
            FsTreeNode::File { rel_path, .. } => out.push(rel_path.clone()),
            FsTreeNode::Directory { children, .. } => collect_tree_files_recursive(children, out),
        }
    }
}

fn is_intellisense_source_path(path: &Path, kind: Option<FileKind>) -> bool {
    if kind == Some(FileKind::Typst) {
        return true;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("typ") || ext.eq_ignore_ascii_case("bib"))
}

/// Intercepts IntelliSense navigation and acceptance keys before `TextEdit` processes input.
///
/// Returns `(consumed, accepted)`. When `consumed` is `true`, caller should skip editor-level
/// shortcuts for this frame.
pub fn handle_intellisense_keys_before_text_edit(
    ctx: &Context,
    editor_has_focus: bool,
    state: &mut EditorIntellisenseState,
) -> (bool, Option<AcceptedCompletion>) {
    if !editor_has_focus {
        return (false, None);
    }
    let Some(req) = state.active_completion.as_ref() else {
        return (false, None);
    };
    if req.items.is_empty() || state.dismissed_at_cursor == Some(req.replace_end_char) {
        return (false, None);
    }

    let len = req.items.len();
    let mut consumed = false;
    let mut accepted = None;
    let mut dismiss = false;

    ctx.input_mut(|i| {
        if i.modifiers.any() {
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            dismiss = true;
            consumed = true;
        } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            state.selected_index = (state.selected_index + 1) % len;
            state.scroll_selection_into_view = true;
            consumed = true;
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            state.selected_index = if state.selected_index == 0 {
                len - 1
            } else {
                state.selected_index - 1
            };
            state.scroll_selection_into_view = true;
            consumed = true;
        } else if i.consume_key(Modifiers::NONE, Key::Tab)
            || i.consume_key(Modifiers::NONE, Key::Enter)
        {
            let idx = state.selected_index.min(len - 1);
            let item = &req.items[idx];
            accepted = Some(AcceptedCompletion {
                replace_start_char: req.replace_start_char,
                replace_end_char: req.replace_end_char,
                insert_text: item.insert_text.clone(),
            });
            consumed = true;
        }
    });

    if dismiss {
        state.dismissed_at_cursor = state.last_cursor_char.or(Some(req.replace_end_char));
        state.active_completion = None;
        state.selected_index = 0;
    } else if accepted.is_some() {
        state.active_completion = None;
        state.selected_index = 0;
        state.dismissed_at_cursor = None;
    }

    (consumed, accepted)
}

/// Parameters for updating and rendering IntelliSense overlays for the active Typst editor.
pub struct IntellisenseUpdateParams<'a> {
    pub active_file: &'a Path,
    pub compiler_diagnostics: &'a [DiagnosticItem],
    pub dark_mode: bool,
    pub ime_in_progress: bool,
    pub accepted_completion: Option<AcceptedCompletion>,
    pub hover_hide_delay_ms: u64,
}

/// Updates completion state, renders the completion popup, paints Typst diagnostic hints,
/// and renders non-colliding hover documentation tooltips for `FileKind::Typst`.
pub fn update_and_render_intellisense(
    ctx: &Context,
    ui: &mut Ui,
    params: IntellisenseUpdateParams<'_>,
    text_ref: &mut String,
    edit_output: &mut TextEditOutput,
    state: &mut EditorIntellisenseState,
    output: &mut EditorPaneOutput,
) {
    if let Some(accepted) = params.accepted_completion {
        apply_accepted_completion(ctx, text_ref, edit_output, &accepted, state, output);
        state.active_hover = None;
    }

    let cursor_char = current_cursor_char(edit_output).unwrap_or(0);
    let cursor_moved = state.last_cursor_char != Some(cursor_char);
    let text_was_edited = edit_output.response.changed() && !params.ime_in_progress;

    if text_was_edited {
        state.active_hover = None;
    }
    if text_was_edited || (cursor_moved && state.dismissed_at_cursor != Some(cursor_char)) {
        state.dismissed_at_cursor = None;
    }

    if (text_was_edited
        || (edit_output.response.has_focus() && cursor_moved && state.active_completion.is_some()))
        && state.dismissed_at_cursor != Some(cursor_char)
    {
        let source_refs = build_source_refs(&state.cached_sources, params.active_file, text_ref);
        let next = compute_completions(text_ref, cursor_char, &source_refs, &state.cached_files)
            .filter(|c| !c.items.is_empty());
        let prev_prefix = state
            .active_completion
            .as_ref()
            .map_or("", |c| c.prefix.as_str());
        let prefix_changed = next.as_ref().is_some_and(|c| c.prefix != prev_prefix);
        state.active_completion = next;
        if prefix_changed {
            state.selected_index = 0;
        } else if let Some(ref req) = state.active_completion {
            state.selected_index = state.selected_index.min(req.items.len().saturating_sub(1));
        } else {
            state.selected_index = 0;
        }
    }

    state.last_cursor_char = Some(cursor_char);

    let mut popup_clicked_item: Option<AcceptedCompletion> = None;
    let mut popup_interacted = false;

    if let Some(req) = state.active_completion.clone() {
        if !req.items.is_empty() && state.dismissed_at_cursor != Some(cursor_char) {
            let (clicked, interacted) = render_completion_popup(
                ctx,
                edit_output,
                &req,
                cursor_char,
                params.dark_mode,
                state,
            );
            popup_clicked_item = clicked;
            popup_interacted = interacted;
        }
    }

    let accepted_by_click = popup_clicked_item.is_some();
    if let Some(accepted) = popup_clicked_item {
        apply_accepted_completion(ctx, text_ref, edit_output, &accepted, state, output);
        state.active_completion = None;
        state.selected_index = 0;
        state.dismissed_at_cursor = None;
        state.active_hover = None;
        state.editor_had_focus = true;
    } else if state.active_completion.is_some()
        && !edit_output.response.has_focus()
        && !edit_output.response.gained_focus()
        && !popup_interacted
    {
        state.active_completion = None;
        state.selected_index = 0;
    }

    let source_refs = build_source_refs(&state.cached_sources, params.active_file, text_ref);
    let mapped_diags =
        map_diagnostics_to_char_spans(text_ref, params.active_file, params.compiler_diagnostics);
    let hints = compute_typst_hints(text_ref, &source_refs, &mapped_diags);
    paint_typst_diagnostic_hints(
        ui,
        edit_output,
        &hints,
        params.dark_mode,
        text_ref.chars().count(),
    );
    if accepted_by_click {
        return;
    }

    state.editor_had_focus = edit_output.response.has_focus()
        || edit_output.response.gained_focus()
        || ctx.memory(|m| m.has_focus(edit_output.response.id));

    if state.active_completion.is_some() || text_was_edited || output.text_changed {
        state.active_hover = None;
    } else {
        update_and_render_intellisense_hover(
            ui,
            edit_output,
            HoverPopupParams {
                active_text: text_ref,
                source_refs: &source_refs,
                mapped_diags: &mapped_diags,
                dark_mode: params.dark_mode,
                hover_hide_delay_ms: params.hover_hide_delay_ms,
            },
            &mut state.active_hover,
        );
    }
}

fn current_cursor_char(edit_output: &TextEditOutput) -> Option<usize> {
    edit_output
        .state
        .cursor
        .char_range()
        .or_else(|| edit_output.cursor_range.map(|cr| cr.as_ccursor_range()))
        .map(|r| r.primary.index)
}

fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn apply_accepted_completion(
    ctx: &Context,
    text_ref: &mut String,
    edit_output: &mut TextEditOutput,
    accepted: &AcceptedCompletion,
    state: &mut EditorIntellisenseState,
    output: &mut EditorPaneOutput,
) {
    let total_chars = text_ref.chars().count();
    let start_char = accepted.replace_start_char.min(total_chars);
    let end_char = accepted.replace_end_char.clamp(start_char, total_chars);
    let start_b = char_to_byte_index(text_ref, start_char);
    let end_b = char_to_byte_index(text_ref, end_char);

    let prev_range = edit_output
        .state
        .cursor
        .char_range()
        .or_else(|| edit_output.cursor_range.map(|cr| cr.as_ccursor_range()))
        .unwrap_or_default();

    let mut undoer = edit_output.state.undoer();
    undoer.feed_state(ctx.input(|i| i.time), &(prev_range, text_ref.clone()));

    text_ref.replace_range(start_b..end_b, &accepted.insert_text);
    let new_cursor_char = start_char + accepted.insert_text.chars().count();
    let new_range = CCursorRange::one(CCursor::new(new_cursor_char));

    edit_output.state.cursor.set_char_range(Some(new_range));
    undoer.feed_state(ctx.input(|i| i.time), &(new_range, text_ref.clone()));
    edit_output.state.set_undoer(undoer);
    edit_output
        .state
        .clone()
        .store(ctx, edit_output.response.id);
    edit_output.response.request_focus();

    output.text_changed = true;
    output.edited_text_snapshot = Some(text_ref.clone());
    output.observed_cursor = Some(CharSelection {
        primary_char: new_cursor_char,
        secondary_char: new_cursor_char,
    });
    state.last_cursor_char = Some(new_cursor_char);
}

fn render_completion_popup(
    ctx: &Context,
    edit_output: &TextEditOutput,
    req: &CompletionRequestContext,
    cursor_char: usize,
    dark_mode: bool,
    state: &mut EditorIntellisenseState,
) -> (Option<AcceptedCompletion>, bool) {
    let palette = ThemePalette::for_mode(dark_mode);
    let popup_id = Id::new("editor_intellisense_completion_popup");

    let visible_rows = req.items.len().clamp(1, 8);
    let list_h = (visible_rows as f32 * 22.0 + 4.0).clamp(32.0, 184.0);
    let body_h = list_h.max(96.0);
    let fallback_h = (body_h + 64.0).min(260.0);

    let caret_cursor = edit_output.galley.from_ccursor(CCursor::new(cursor_char));
    let caret_screen = edit_output
        .galley
        .pos_from_cursor(&caret_cursor)
        .translate(edit_output.galley_pos.to_vec2());
    let screen_rect = ctx.screen_rect();
    let max_total_w = (screen_rect.width() - 24.0).clamp(280.0, 540.0);
    let area_size = AreaState::load(ctx, popup_id).and_then(|s| s.size);
    let est_width = area_size
        .map_or(max_total_w, |sz| sz.x)
        .min((screen_rect.width() - 8.0).max(120.0));
    let est_height = area_size.map_or(fallback_h, |sz| sz.y.min(280.0));
    let max_anchor_x = (screen_rect.right() - est_width - 4.0).max(screen_rect.left() + 4.0);
    let anchor_x = caret_screen
        .min
        .x
        .clamp(screen_rect.left() + 4.0, max_anchor_x);

    let place_below = caret_screen.max.y + 4.0 + est_height <= screen_rect.bottom()
        || caret_screen.min.y - screen_rect.top() < est_height + 4.0;
    let (pivot, anchor) = if place_below {
        (
            Align2::LEFT_TOP,
            Pos2::new(anchor_x, caret_screen.max.y + 4.0),
        )
    } else {
        (
            Align2::LEFT_BOTTOM,
            Pos2::new(anchor_x, caret_screen.min.y - 4.0),
        )
    };

    let selected_idx = state.selected_index.min(req.items.len().saturating_sub(1));
    let scroll_to_sel = std::mem::take(&mut state.scroll_selection_into_view);
    let mut clicked_item: Option<AcceptedCompletion> = None;

    let area_resp = Area::new(popup_id)
        .kind(UiKind::Popup)
        .order(Order::Foreground)
        .pivot(pivot)
        .fixed_pos(anchor)
        .constrain(true)
        .show(ctx, |ui| {
            Frame::popup(&ctx.style())
                .fill(palette.chrome_sidebar)
                .stroke(Stroke::new(1.0_f32, palette.panel_border))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_max_width(max_total_w);
                    ui.label(
                        RichText::new(t!("intellisense.suggestions_header"))
                            .small()
                            .strong()
                            .color(palette.text_muted),
                    );
                    ui.separator();

                    let list_w = (max_total_w * 0.48).max(160.0);
                    let doc_w = (max_total_w - list_w - 20.0).max(140.0);

                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(list_w, body_h),
                            egui::Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_max_height(body_h);
                                ScrollArea::vertical()
                                    .id_salt("intellisense_candidates_scroll")
                                    .max_height(body_h)
                                    .show(ui, |ui| {
                                        for (idx, item) in req.items.iter().enumerate() {
                                            let is_sel = idx == selected_idx;
                                            let (badge, badge_col) =
                                                completion_kind_badge(item.kind, &palette);
                                            let row_resp = ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(badge)
                                                        .monospace()
                                                        .small()
                                                        .color(badge_col),
                                                );
                                                let btn = ui.selectable_label(
                                                    is_sel,
                                                    RichText::new(&item.label).monospace().strong(),
                                                );
                                                if !item.detail.is_empty() {
                                                    ui.label(
                                                        RichText::new(&item.detail)
                                                            .small()
                                                            .color(palette.text_muted),
                                                    );
                                                }
                                                btn
                                            });
                                            let item_resp = row_resp.inner;
                                            if is_sel && scroll_to_sel {
                                                item_resp.scroll_to_me(Some(Align::Center));
                                            }
                                            if item_resp.clicked() {
                                                clicked_item = Some(AcceptedCompletion {
                                                    replace_start_char: req.replace_start_char,
                                                    replace_end_char: req.replace_end_char,
                                                    insert_text: item.insert_text.clone(),
                                                });
                                            }
                                        }
                                    });
                            },
                        );

                        let (sep_rect, _) =
                            ui.allocate_exact_size(Vec2::new(1.0, body_h), Sense::hover());
                        ui.painter().line_segment(
                            [
                                Pos2::new(sep_rect.center().x, sep_rect.top() + 2.0),
                                Pos2::new(sep_rect.center().x, sep_rect.bottom() - 2.0),
                            ],
                            Stroke::new(1.0_f32, palette.panel_border),
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(doc_w, body_h),
                            egui::Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_max_height(body_h);
                                render_completion_doc_pane(
                                    ui,
                                    &req.items[selected_idx],
                                    &palette,
                                    body_h,
                                );
                            },
                        );
                    });

                    ui.separator();
                    ui.label(
                        RichText::new(t!("intellisense.hint_accept"))
                            .small()
                            .color(palette.text_muted),
                    );
                });
        });

    let pointer_in_popup = ctx
        .input(|i| i.pointer.interact_pos())
        .is_some_and(|p| area_resp.response.rect.contains(p));
    let popup_interacted =
        clicked_item.is_some() || area_resp.response.hovered() || pointer_in_popup;

    (clicked_item, popup_interacted)
}
