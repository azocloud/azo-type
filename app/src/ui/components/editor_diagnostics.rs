//! Editor overlay rendering spell, grammar, and typography underlines plus hover tooltips.

use egui::text::CCursor;
use egui::widgets::text_edit::TextEditOutput;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui};

use crate::core::spellcheck::{DiagnosticCategory, LanguageCheckDiagnostic};
use crate::ui::theme::ThemePalette;

#[cfg(test)]
mod tests;

/// Returns the semantic underline color for a language diagnostic category.
pub fn diagnostic_color(category: DiagnosticCategory, dark_mode: bool) -> Color32 {
    match category {
        DiagnosticCategory::Spelling => {
            if dark_mode {
                Color32::from_rgb(248, 113, 113)
            } else {
                Color32::from_rgb(220, 38, 38)
            }
        }
        DiagnosticCategory::Grammar => {
            if dark_mode {
                Color32::from_rgb(96, 165, 250)
            } else {
                Color32::from_rgb(37, 99, 235)
            }
        }
        DiagnosticCategory::Typography => {
            if dark_mode {
                Color32::from_rgb(251, 191, 36)
            } else {
                Color32::from_rgb(217, 119, 6)
            }
        }
    }
}

/// Builds zigzag/wave vertices between `x0` and `x1` centered vertically around `y_base`.
pub fn compute_wavy_points(x0: f32, x1: f32, y_base: f32) -> Vec<Pos2> {
    if x1 <= x0 + 1.0 {
        return Vec::new();
    }
    let step = 3.0_f32;
    let amp = 1.1_f32;
    let mut pts = Vec::new();
    let mut x = x0;
    let mut up = false;
    pts.push(Pos2::new(x, y_base));
    while x + step < x1 {
        x += step;
        up = !up;
        let y = if up { y_base - amp } else { y_base + amp };
        pts.push(Pos2::new(x, y));
    }
    let final_y = if !up { y_base - amp * 0.5 } else { y_base };
    pts.push(Pos2::new(x1, final_y));
    pts
}

/// Paints spell/grammar/typography underlines under matching ranges in the editor galley
/// and displays a concise hover tooltip when the pointer rests over an underlined span.
pub fn paint_language_diagnostics(
    ui: &Ui,
    edit_output: &TextEditOutput,
    diagnostics: &[LanguageCheckDiagnostic],
    dark_mode: bool,
    total_chars: usize,
) {
    if diagnostics.is_empty() || total_chars == 0 {
        return;
    }

    let galley = &edit_output.galley;
    let galley_pos = edit_output.galley_pos;
    let painter = ui.painter();
    let clip_rect = painter.clip_rect();
    let palette = ThemePalette::for_mode(dark_mode);

    let mut row_ranges: Vec<(usize, usize, Rect)> = Vec::with_capacity(galley.rows.len());
    let mut row_start = 0usize;
    for row in &galley.rows {
        let count = row.char_count_including_newline();
        let visible_end = row_start + count.saturating_sub(usize::from(row.ends_with_newline));
        let row_rect = row.rect.translate(galley_pos.to_vec2());
        row_ranges.push((row_start, visible_end, row_rect));
        row_start += count;
    }

    let mut hovered_diag: Option<&LanguageCheckDiagnostic> = None;

    for diag in diagnostics {
        if diag.start_char >= diag.end_char || diag.start_char >= total_chars {
            continue;
        }
        let d_start = diag.start_char;
        let d_end = diag.end_char.min(total_chars);
        let color = diagnostic_color(diag.category, dark_mode);

        for &(r_start, r_visible_end, r_rect) in &row_ranges {
            if r_rect.max.y < clip_rect.min.y || r_rect.min.y > clip_rect.max.y {
                continue;
            }
            if d_end <= r_start || d_start >= r_visible_end {
                continue;
            }

            let seg_start = d_start.max(r_start);
            let seg_end = d_end.min(r_visible_end);
            if seg_end <= seg_start {
                continue;
            }

            let start_rect = galley
                .pos_from_cursor(&galley.from_ccursor(CCursor::new(seg_start)))
                .translate(galley_pos.to_vec2());
            let mut end_rect = galley
                .pos_from_cursor(&galley.from_ccursor(CCursor::new(seg_end)))
                .translate(galley_pos.to_vec2());

            // At a soft-wrap boundary, end_rect may resolve to index 0 of the next visual row.
            if (end_rect.min.y - start_rect.min.y).abs() > 2.0 || end_rect.min.x <= start_rect.min.x
            {
                end_rect.min.x = r_rect.max.x;
            }

            let x0 = start_rect.min.x;
            let x1 = end_rect.min.x.max(x0 + 2.0);
            let y_base = (r_rect.max.y - 1.5).max(r_rect.min.y + 2.0);

            match diag.category {
                DiagnosticCategory::Spelling => {
                    let pts = compute_wavy_points(x0, x1, y_base);
                    if pts.len() >= 2 {
                        painter.line(pts, Stroke::new(1.3_f32, color));
                    }
                }
                DiagnosticCategory::Grammar | DiagnosticCategory::Typography => {
                    painter.line_segment(
                        [Pos2::new(x0, y_base), Pos2::new(x1, y_base)],
                        Stroke::new(1.6_f32, color),
                    );
                }
            }

            let hit_rect = Rect::from_min_max(
                Pos2::new(x0, r_rect.min.y),
                Pos2::new(x1, r_rect.max.y + 1.0),
            );
            if hovered_diag.is_none() && ui.rect_contains_pointer(hit_rect) {
                hovered_diag = Some(diag);
            }
        }
    }

    if let Some(diag) = hovered_diag {
        egui::show_tooltip_at_pointer(
            ui.ctx(),
            ui.layer_id(),
            egui::Id::new("editor_language_diag_tooltip"),
            |ui| {
                ui.set_max_width(340.0);
                let badge = match diag.category {
                    DiagnosticCategory::Spelling => "Spelling",
                    DiagnosticCategory::Grammar => "Grammar",
                    DiagnosticCategory::Typography => "Style",
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(badge)
                            .strong()
                            .small()
                            .color(diagnostic_color(diag.category, dark_mode)),
                    );
                    if !diag.excerpt.is_empty() {
                        ui.label(
                            RichText::new(format!("“{}”", diag.excerpt))
                                .monospace()
                                .small()
                                .color(palette.text_muted),
                        );
                    }
                });
                ui.label(RichText::new(&diag.message).small());
                if !diag.suggestions.is_empty() {
                    let preview = diag
                        .suggestions
                        .iter()
                        .take(4)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    ui.label(
                        RichText::new(format!("→ {preview}"))
                            .small()
                            .color(palette.syn_string),
                    );
                }
            },
        );
    }
}
