//! Typst diagnostic hint underlines and non-colliding IntelliSense hover tooltips.

use egui::text::CCursor;
use egui::widgets::text_edit::TextEditOutput;
use egui::{
    Align2, Area, AreaState, Color32, Context, Frame, Id, Order, Pos2, Rect, RichText, Sense,
    Stroke, Ui, UiKind, Vec2,
};
use rust_i18n::t;

use crate::core::diff::normalize_rel_path;
use crate::core::intellisense::{CompletionItem, CompletionKind, HoverInfo, TypstDiagnosticHint};
use crate::engine::DiagnosticItem;
use crate::ui::theme::ThemePalette;

/// Returns the compact badge text and accent color for a completion item kind.
pub(super) fn completion_kind_badge(
    kind: CompletionKind,
    palette: &ThemePalette,
) -> (&'static str, Color32) {
    match kind {
        CompletionKind::Reference => ("[Ref]", palette.syn_string),
        CompletionKind::Macro => ("[Macro]", palette.syn_property_or_func),
        CompletionKind::Function => ("[Fn]", palette.syn_keyword),
        CompletionKind::Keyword => ("[Kw]", palette.syn_operator),
        CompletionKind::FilePath => ("[File]", palette.syn_number),
        CompletionKind::Variable => ("[Var]", palette.text_primary),
        CompletionKind::Parameter => ("[Param]", palette.syn_string),
    }
}

/// Renders the documentation and source context preview pane for the selected completion item.
pub(super) fn render_completion_doc_pane(
    ui: &mut Ui,
    item: &CompletionItem,
    palette: &ThemePalette,
) {
    ui.label(
        RichText::new(&item.label)
            .monospace()
            .strong()
            .color(palette.text_primary),
    );
    if let Some(ref hint) = item.source_hint {
        ui.label(
            RichText::new(hint)
                .small()
                .monospace()
                .color(palette.syn_keyword),
        );
    }
    if !item.detail.is_empty() {
        ui.label(
            RichText::new(&item.detail)
                .small()
                .color(palette.text_muted),
        );
    }
    ui.separator();
    let doc_text = if item.documentation.trim().is_empty() {
        t!("intellisense.no_docs").to_string()
    } else {
        item.documentation.clone()
    };
    egui::ScrollArea::vertical()
        .id_salt("intellisense_doc_scroll")
        .max_height(130.0)
        .show(ui, |ui| {
            ui.label(RichText::new(doc_text).small().color(palette.text_primary));
        });
}

/// Maps compiler `DiagnosticItem`s for `active_file` from 1-based `(line, column)` to char spans.
pub fn map_diagnostics_to_char_spans(
    active_text: &str,
    active_file: &std::path::Path,
    diagnostics: &[DiagnosticItem],
) -> Vec<(usize, usize, String, bool)> {
    let norm_active = normalize_rel_path(&active_file.to_string_lossy().replace('\\', "/"));
    let total_chars = active_text.chars().count();
    let mut out = Vec::new();

    for diag in diagnostics {
        let norm_diag = normalize_rel_path(&diag.file.to_string_lossy().replace('\\', "/"));
        let file_matches = norm_diag == norm_active
            || (!norm_active.is_empty() && norm_diag.ends_with(&format!("/{norm_active}")));
        if !file_matches {
            continue;
        }
        let Some(line_1based) = diag.line else {
            continue;
        };
        let (start_char, end_char) =
            line_col_to_char_span(active_text, line_1based, diag.column, total_chars);
        let msg = if diag.hints.is_empty() {
            diag.message.clone()
        } else {
            format!("{}\n{}", diag.message, diag.hints.join("\n"))
        };
        out.push((start_char, end_char, msg, diag.is_error));
    }

    out
}

fn line_col_to_char_span(
    text: &str,
    line_1based: usize,
    col_1based: Option<usize>,
    total_chars: usize,
) -> (usize, usize) {
    let target_idx = line_1based.saturating_sub(1);
    let mut offset = 0usize;

    for (idx, line) in text.split_inclusive('\n').enumerate() {
        if idx == target_idx {
            let trimmed = line
                .strip_suffix('\n')
                .unwrap_or(line)
                .strip_suffix('\r')
                .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line));
            let line_chars = trimmed.chars().count();
            let col_0 = col_1based.unwrap_or(1).saturating_sub(1).min(line_chars);
            let start = offset + col_0;
            let span_len = token_char_len_at(trimmed, col_0);
            let end = (start + span_len).min(offset + line_chars);
            return (start, end);
        }
        offset += line.chars().count();
    }

    (total_chars, total_chars)
}

fn token_char_len_at(line: &str, col_0: usize) -> usize {
    let mut iter = line.chars().skip(col_0);
    let Some(first) = iter.next() else {
        return 0;
    };
    if first == '@' || first == '#' || first.is_alphanumeric() || first == '_' || first == '-' {
        1 + iter
            .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ':'))
            .count()
    } else {
        1
    }
}

/// Paints subtle dashed underlines for Typst compiler diagnostics and unresolved `@ref` hints.
pub fn paint_typst_diagnostic_hints(
    ui: &Ui,
    edit_output: &TextEditOutput,
    hints: &[TypstDiagnosticHint],
    dark_mode: bool,
    total_chars: usize,
) {
    if hints.is_empty() || total_chars == 0 {
        return;
    }

    let galley = &edit_output.galley;
    let galley_pos = edit_output.galley_pos;
    let painter = ui.painter();
    let clip_rect = painter.clip_rect();

    let mut row_ranges: Vec<(usize, usize, Rect)> = Vec::with_capacity(galley.rows.len());
    let mut row_start = 0usize;
    for row in &galley.rows {
        let count = row.char_count_including_newline();
        let visible_end = row_start + count.saturating_sub(usize::from(row.ends_with_newline));
        let row_rect = row.rect.translate(galley_pos.to_vec2());
        row_ranges.push((row_start, visible_end, row_rect));
        row_start += count;
    }

    for hint in hints {
        if hint.start_char >= total_chars {
            continue;
        }
        let h_start = hint.start_char;
        let h_end = hint.end_char.max(h_start + 1).min(total_chars);
        let color = if hint.is_error {
            if dark_mode {
                Color32::from_rgb(244, 63, 94)
            } else {
                Color32::from_rgb(190, 18, 60)
            }
        } else if dark_mode {
            Color32::from_rgb(168, 85, 247)
        } else {
            Color32::from_rgb(126, 34, 206)
        };

        for &(r_start, r_visible_end, r_rect) in &row_ranges {
            if r_rect.max.y < clip_rect.min.y || r_rect.min.y > clip_rect.max.y {
                continue;
            }
            if h_end <= r_start || h_start >= r_visible_end {
                continue;
            }
            let seg_start = h_start.max(r_start);
            let seg_end = h_end.min(r_visible_end);
            if seg_end <= seg_start {
                continue;
            }

            let start_rect = galley
                .pos_from_cursor(&galley.from_ccursor(CCursor::new(seg_start)))
                .translate(galley_pos.to_vec2());
            let mut end_rect = galley
                .pos_from_cursor(&galley.from_ccursor(CCursor::new(seg_end)))
                .translate(galley_pos.to_vec2());
            if (end_rect.min.y - start_rect.min.y).abs() > 2.0 || end_rect.min.x <= start_rect.min.x
            {
                end_rect.min.x = r_rect.max.x;
            }

            let x0 = start_rect.min.x;
            let x1 = end_rect.min.x.max(x0 + 3.0);
            let y = (r_rect.max.y - 0.5).max(r_rect.min.y + 2.0);
            paint_dashed_line(painter, x0, x1, y, Stroke::new(1.4_f32, color));
        }
    }
}

fn paint_dashed_line(painter: &egui::Painter, x0: f32, x1: f32, y: f32, stroke: Stroke) {
    let mut x = x0;
    while x < x1 {
        let seg_end = (x + 3.0).min(x1);
        painter.line_segment([Pos2::new(x, y), Pos2::new(seg_end, y)], stroke);
        x += 5.0;
    }
}

/// Resolves the character index currently under the pointer inside `edit_output.galley`.
pub fn hovered_char_in_galley(
    ctx: &Context,
    ui: &Ui,
    edit_output: &TextEditOutput,
) -> Option<usize> {
    let galley_rect = edit_output
        .galley
        .rect
        .translate(edit_output.galley_pos.to_vec2());
    if !ui.rect_contains_pointer(galley_rect) {
        return None;
    }
    let pointer_pos = ctx.input(|i| i.pointer.hover_pos())?;
    let in_row = edit_output.galley.rows.iter().any(|row| {
        let r = row.rect.translate(edit_output.galley_pos.to_vec2());
        pointer_pos.y >= r.min.y
            && pointer_pos.y <= r.max.y
            && pointer_pos.x >= r.min.x - 2.0
            && pointer_pos.x <= r.max.x + 2.0
    });
    if !in_row {
        return None;
    }

    let rel_pos = pointer_pos - edit_output.galley_pos;
    let hover_cursor = edit_output.galley.cursor_from_pos(rel_pos);
    let cursor_x =
        edit_output.galley.pos_from_cursor(&hover_cursor).min.x + edit_output.galley_pos.x;
    let idx = if cursor_x > pointer_pos.x {
        hover_cursor.ccursor.index.saturating_sub(1)
    } else {
        hover_cursor.ccursor.index
    };
    Some(idx)
}

/// Renders the IntelliSense hover popup near the hovered token without overlapping
/// `editor_diagnostics::paint_language_diagnostics`'s `show_tooltip_at_pointer` tooltip.
pub fn render_intellisense_hover(
    ui: &Ui,
    edit_output: &TextEditOutput,
    hover: &HoverInfo,
    dark_mode: bool,
) {
    let ctx = ui.ctx();
    let palette = ThemePalette::for_mode(dark_mode);
    let hover_area_id = Id::new("editor_intellisense_hover_popup");

    let start_rect = edit_output
        .galley
        .pos_from_cursor(
            &edit_output
                .galley
                .from_ccursor(CCursor::new(hover.start_char)),
        )
        .translate(edit_output.galley_pos.to_vec2());
    let token_rect = Rect::from_min_max(
        start_rect.min,
        Pos2::new(
            start_rect.max.x.max(start_rect.min.x + 8.0),
            start_rect.max.y,
        ),
    );

    let diag_widget_id = Id::new("editor_language_diag_tooltip");
    let diag_area_id = egui::tooltip_id(diag_widget_id, 0);
    let diag_active_this_frame = egui::next_tooltip_id(ctx, diag_widget_id) != diag_area_id;
    let diag_rect =
        if diag_active_this_frame || egui::was_tooltip_open_last_frame(ctx, diag_widget_id) {
            AreaState::load(ctx, diag_area_id).map(|s| s.rect())
        } else {
            None
        };

    let screen_rect = ctx.screen_rect();
    let est_size = AreaState::load(ctx, hover_area_id)
        .and_then(|s| s.size)
        .unwrap_or(Vec2::new(260.0, 88.0));

    let (pivot, anchor) =
        compute_hover_popup_placement(screen_rect, token_rect, diag_rect, est_size);

    Area::new(hover_area_id)
        .kind(UiKind::Popup)
        .order(Order::Tooltip)
        .pivot(pivot)
        .fixed_pos(anchor)
        .constrain(true)
        .sense(Sense::hover())
        .show(ctx, |ui| {
            Frame::popup(&ctx.style())
                .fill(palette.chrome_sidebar)
                .stroke(Stroke::new(1.0_f32, palette.panel_border))
                .show(ui, |ui| {
                    ui.set_max_width(340.0);
                    ui.horizontal(|ui| {
                        if hover.is_diagnostic {
                            let badge_col = if dark_mode {
                                Color32::from_rgb(244, 63, 94)
                            } else {
                                Color32::from_rgb(190, 18, 60)
                            };
                            ui.label(
                                RichText::new(t!("intellisense.diagnostic_badge"))
                                    .strong()
                                    .small()
                                    .color(badge_col),
                            );
                        }
                        ui.label(
                            RichText::new(&hover.title)
                                .monospace()
                                .strong()
                                .small()
                                .color(palette.text_primary),
                        );
                    });
                    if !hover.detail.is_empty() {
                        ui.label(
                            RichText::new(&hover.detail)
                                .small()
                                .color(palette.syn_keyword),
                        );
                    }
                    if !hover.documentation.trim().is_empty() {
                        ui.separator();
                        ui.label(
                            RichText::new(&hover.documentation)
                                .small()
                                .color(palette.text_primary),
                        );
                    } else if hover.detail.is_empty() {
                        ui.label(
                            RichText::new(t!("intellisense.no_docs"))
                                .small()
                                .color(palette.text_muted),
                        );
                    }
                });
        });
}

fn compute_hover_popup_placement(
    screen_rect: Rect,
    token_rect: Rect,
    diag_rect: Option<Rect>,
    est_size: Vec2,
) -> (Align2, Pos2) {
    let fits_above_token = token_rect.min.y - screen_rect.top() >= est_size.y + 8.0
        && diag_rect.is_none_or(|dr| dr.min.y >= token_rect.min.y);
    if fits_above_token {
        return (
            Align2::LEFT_BOTTOM,
            Pos2::new(token_rect.min.x, token_rect.min.y - 6.0),
        );
    }

    if let Some(dr) = diag_rect {
        if dr.max.y + 6.0 + est_size.y <= screen_rect.bottom() {
            return (Align2::LEFT_TOP, Pos2::new(dr.min.x, dr.max.y + 6.0));
        }
        if dr.max.x + 8.0 + est_size.x <= screen_rect.right() {
            return (Align2::LEFT_TOP, Pos2::new(dr.max.x + 8.0, dr.min.y));
        }
        return (Align2::LEFT_BOTTOM, Pos2::new(dr.min.x, dr.min.y - 6.0));
    }

    (
        Align2::LEFT_TOP,
        Pos2::new(token_rect.min.x, token_rect.max.y + 6.0),
    )
}
