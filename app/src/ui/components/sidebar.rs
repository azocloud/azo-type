//! Collapsible sidebar displaying the disk project tree and the document outline.

use std::path::{Path, PathBuf};

use egui::{
    Align, CollapsingHeader, Color32, Context, CornerRadius, Frame, Layout, Margin, Pos2, RichText,
    ScrollArea, Sense, SidePanel, Stroke, Ui, UiBuilder, Vec2,
};
use rust_i18n::t;

use crate::collab::awareness::PeerPresence;
use crate::core::diff::{normalize_rel_path, CharSelection};
use crate::core::file_kind::FileKind;
use crate::core::outline::{
    build_outline_tree, extract_document_outline, CompiledOutlineItem, DocumentOutlineItem,
    OutlineTreeNode,
};
use crate::project::{DiskProject, FsTreeNode};
use crate::ui::theme::ThemePalette;

/// Output actions triggered by user interactions in the sidebar.
#[derive(Debug, Default)]
pub struct SidebarOutput {
    pub rescan_tree: bool,
    pub show_new_file_modal: bool,
    pub navigate_to_item: Option<DocumentOutlineItem>,
    pub clicked_path: Option<PathBuf>,
    pub new_main_path: Option<PathBuf>,
    pub rename_req: Option<PathBuf>,
    pub delete_req: Option<PathBuf>,
}

/// Parameters for rendering the sidebar panel.
pub struct SidebarParams<'a> {
    pub dark_mode: bool,
    pub show_sidebar: bool,
    pub sidebar_split_ratio: &'a mut f32,
    pub project: &'a DiskProject,
    pub remote_presences: &'a [PeerPresence],
    pub compiled_outline: &'a [CompiledOutlineItem],
    pub last_editor_selection: Option<CharSelection>,
}

/// Renders the collapsible left sidebar containing the project file tree and document outline.
pub fn render_sidebar(ctx: &Context, params: SidebarParams<'_>) -> SidebarOutput {
    let mut output = SidebarOutput::default();

    if !params.show_sidebar {
        return output;
    }

    let palette = ThemePalette::for_mode(params.dark_mode);
    let panel_bg = palette.chrome_sidebar;
    let border_col = palette.panel_border;

    SidePanel::left("left_file_tree_panel")
        .resizable(true)
        .default_width(265.0)
        .width_range(195.0..=450.0)
        .frame(
            Frame::new()
                .fill(panel_bg)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(10, 10))
                .stroke(Stroke::new(1.0_f32, border_col)),
        )
        .show(ctx, |ui| {
            let total_rect = ui.available_rect_before_wrap();
            let split = crate::ui::components::compute_horizontal_split(
                total_rect,
                8.0,
                110.0,
                110.0,
                *params.sidebar_split_ratio,
            );

            // Top Panel: Project Files Tree
            let mut top_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(split.first_rect)
                    .layout(Layout::top_down(Align::Min)),
            );
            top_ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("sidebar.disk_tree_title"))
                        .size(11.5)
                        .strong()
                        .color(palette.text_muted),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .small_button(t!("sidebar.refresh_button"))
                        .on_hover_text(t!("sidebar.refresh_tooltip"))
                        .clicked()
                    {
                        output.rescan_tree = true;
                    }
                    if ui
                        .small_button(t!("sidebar.new_file_button"))
                        .on_hover_text(t!("sidebar.new_file_tooltip"))
                        .clicked()
                    {
                        output.show_new_file_modal = true;
                    }
                });
            });

            top_ui.add_space(6.0);

            let active_path = params.project.active_file.clone();
            let main_path = params.project.main_file.clone();
            let project_title = t!("sidebar.folder_prefix", name = &params.project.project_name);
            let tree_snapshot = params.project.tree.clone();

            ScrollArea::vertical()
                .id_salt("file_tree_scroll")
                .max_height(top_ui.available_height())
                .auto_shrink([false, false])
                .show(&mut top_ui, |ui| {
                    CollapsingHeader::new(
                        RichText::new(project_title)
                            .strong()
                            .color(palette.text_primary),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        for node in &tree_snapshot {
                            render_fs_tree_node(
                                ui,
                                node,
                                params.project,
                                &palette,
                                &active_path,
                                &main_path,
                                params.remote_presences,
                                &mut output.clicked_path,
                                &mut output.new_main_path,
                                &mut output.rename_req,
                                &mut output.delete_req,
                            );
                        }
                    });
                });

            // Bottom Panel: Document Outline Hierarchy for the active editor file
            let mut bottom_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(split.second_rect)
                    .layout(Layout::top_down(Align::Min)),
            );
            bottom_ui.add_space(2.0);

            let outline_items = params
                .project
                .active_buffer()
                .and_then(|buf| {
                    buf.text.as_deref().map(|txt| {
                        extract_document_outline(
                            txt,
                            buf.kind,
                            &buf.rel_path,
                            Some(params.project),
                            params.compiled_outline,
                        )
                    })
                })
                .unwrap_or_default();

            bottom_ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("sidebar.outline_title"))
                        .size(11.5)
                        .strong()
                        .color(palette.text_muted),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let count_str = t!("sidebar.headings_count", count = outline_items.len());
                    ui.label(
                        RichText::new(count_str)
                            .size(10.5)
                            .monospace()
                            .color(palette.text_muted),
                    );
                });
            });

            bottom_ui.add_space(4.0);

            let current_cursor_char = params
                .last_editor_selection
                .map(|s| s.primary_char)
                .unwrap_or(0);
            let active_outline_idx = outline_items
                .iter()
                .enumerate()
                .rev()
                .find(|(_, item)| {
                    item.file == active_path && item.char_offset <= current_cursor_char
                })
                .map(|(idx, _)| idx);

            let outline_tree = build_outline_tree(&outline_items, active_outline_idx);

            Frame::new()
                .fill(palette.editor_canvas)
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(8, 8))
                .stroke(Stroke::new(1.0_f32, border_col))
                .show(&mut bottom_ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("document_outline_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if outline_tree.is_empty() {
                                ui.label(
                                    RichText::new(t!("sidebar.no_headings"))
                                        .small()
                                        .color(palette.text_muted),
                                );
                            } else {
                                let root_len = outline_tree.len();
                                for (idx, root_node) in outline_tree.iter().enumerate() {
                                    render_outline_tree_node(
                                        ui,
                                        root_node,
                                        0,
                                        idx + 1 == root_len,
                                        &active_path,
                                        &palette,
                                        &mut output.navigate_to_item,
                                    );
                                }
                            }
                        });
                });

            crate::ui::components::render_horizontal_split_divider(
                ctx,
                ui,
                &palette,
                "sidebar_tree_outline_split_divider",
                "Drag to resize Project Files and Outline panels (double-click to reset)",
                total_rect,
                &split,
                0.42,
                params.sidebar_split_ratio,
            );
        });

    output
}

fn render_outline_tree_node(
    ui: &mut Ui,
    node: &OutlineTreeNode,
    depth: usize,
    is_last: bool,
    active_file: &Path,
    palette: &ThemePalette,
    navigate_to_item: &mut Option<DocumentOutlineItem>,
) {
    let has_children = !node.children.is_empty();
    let collapse_id = ui.make_persistent_id((
        "outline_tree_node",
        &node.item.file,
        node.item.line_number,
        &node.item.title,
    ));
    let mut is_open = ui
        .ctx()
        .data_mut(|d| *d.get_persisted_mut_or(collapse_id, true));

    let indent_w = (depth as f32) * 14.0;
    let row_resp = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if indent_w > 0.0 {
            let (guide_rect, _) = ui.allocate_exact_size(Vec2::new(indent_w, 18.0), Sense::hover());
            // Draw vertical hierarchy guide lines for ancestor levels
            for lvl in 0..depth {
                let gx = guide_rect.min.x + (lvl as f32) * 14.0 + 6.0;
                let max_y = if lvl + 1 == depth && is_last && !has_children {
                    guide_rect.center().y
                } else {
                    guide_rect.max.y
                };
                ui.painter().line_segment(
                    [Pos2::new(gx, guide_rect.min.y), Pos2::new(gx, max_y)],
                    Stroke::new(1.0_f32, palette.panel_border),
                );
                if lvl + 1 == depth {
                    ui.painter().line_segment(
                        [
                            Pos2::new(gx, guide_rect.center().y),
                            Pos2::new(gx + 6.0, guide_rect.center().y),
                        ],
                        Stroke::new(1.0_f32, palette.panel_border),
                    );
                }
            }
        }

        if has_children {
            let arrow = if is_open { "▼" } else { "▶" };
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(arrow)
                            .size(9.5)
                            .monospace()
                            .color(palette.text_muted),
                    )
                    .frame(false)
                    .min_size(Vec2::new(12.0, 16.0)),
                )
                .on_hover_text(if is_open {
                    t!("sidebar.collapse_subheadings")
                } else {
                    t!("sidebar.expand_subheadings")
                })
                .clicked()
            {
                is_open = !is_open;
                ui.ctx()
                    .data_mut(|d| d.insert_persisted(collapse_id, is_open));
            }
        } else {
            ui.add_space(2.0);
            ui.label(
                RichText::new("•")
                    .size(10.5)
                    .monospace()
                    .color(palette.text_muted),
            );
        }

        // Typst hierarchy badge (`= 1`, `== 1.1`, `=== 1.2.1`)
        let badge_color = match node.item.level {
            1 => palette.syn_keyword,
            2 => palette.syn_property_or_func,
            3 => palette.syn_string,
            _ => palette.syn_operator,
        };
        let eq_prefix = "=".repeat(node.item.level.min(4));
        let badge_text = if node.item.number_label.is_empty() {
            eq_prefix
        } else {
            format!("{} {}", eq_prefix, node.item.number_label)
        };
        ui.label(
            RichText::new(badge_text)
                .size(10.0)
                .monospace()
                .strong()
                .color(badge_color),
        );

        let is_external_file = node.item.file != active_file;
        let max_chars = if is_external_file {
            16usize.saturating_sub(depth * 2)
        } else {
            20usize.saturating_sub(depth * 2)
        }
        .max(10);
        let display_title = if node.item.title.chars().count() > max_chars {
            let truncated: String = node
                .item
                .title
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect();
            format!("{}…", truncated.trim_end())
        } else {
            node.item.title.clone()
        };

        let mut title_rt = RichText::new(display_title)
            .size(11.8)
            .color(if node.is_current {
                palette.text_primary
            } else if node.item.level == 1 {
                palette.text_primary.gamma_multiply(0.92)
            } else {
                palette.text_muted
            });
        if node.is_current || node.item.level == 1 {
            title_rt = title_rt.strong();
        }

        let hover_msg = if is_external_file {
            format!(
                "{} {} — {}\n{}",
                "=".repeat(node.item.level),
                node.item.number_label,
                node.item.title,
                t!(
                    "sidebar.jump_external_tooltip",
                    file = node.item.file.display().to_string(),
                    line = node.item.line_number
                )
            )
        } else {
            format!(
                "{} {} — {}\n{}",
                "=".repeat(node.item.level),
                node.item.number_label,
                node.item.title,
                t!("sidebar.jump_local_tooltip", line = node.item.line_number)
            )
        };

        let resp = ui
            .selectable_label(node.is_current, title_rt)
            .on_hover_text(hover_msg);
        if resp.clicked() {
            *navigate_to_item = Some(node.item.clone());
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let loc_label = if is_external_file {
                let short_f = node
                    .item
                    .file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("ext");
                format!("{}:L{}", short_f, node.item.line_number)
            } else {
                format!("L{}", node.item.line_number)
            };
            ui.label(
                RichText::new(loc_label)
                    .size(9.5)
                    .monospace()
                    .color(palette.text_muted),
            );
        });
    });

    let _ = row_resp;

    if has_children && is_open {
        let child_len = node.children.len();
        for (c_idx, child) in node.children.iter().enumerate() {
            render_outline_tree_node(
                ui,
                child,
                depth + 1,
                c_idx + 1 == child_len,
                active_file,
                palette,
                navigate_to_item,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_fs_tree_node(
    ui: &mut Ui,
    node: &FsTreeNode,
    project: &DiskProject,
    palette: &ThemePalette,
    active_path: &Path,
    main_path: &Path,
    remote_presences: &[PeerPresence],
    clicked_path: &mut Option<PathBuf>,
    new_main_path: &mut Option<PathBuf>,
    rename_req: &mut Option<PathBuf>,
    delete_req: &mut Option<PathBuf>,
) {
    match node {
        FsTreeNode::Directory {
            name,
            rel_path: _,
            children,
        } => {
            let header = format!("{}/", name);
            CollapsingHeader::new(RichText::new(header).strong().color(palette.text_primary))
                .default_open(true)
                .show(ui, |ui| {
                    for child in children {
                        render_fs_tree_node(
                            ui,
                            child,
                            project,
                            palette,
                            active_path,
                            main_path,
                            remote_presences,
                            clicked_path,
                            new_main_path,
                            rename_req,
                            delete_req,
                        );
                    }
                });
        }
        FsTreeNode::File {
            name,
            rel_path,
            abs_path,
            kind,
            ..
        } => {
            let is_active = rel_path == active_path;
            let is_main = rel_path == main_path;
            let dirty = project
                .buffers
                .get(rel_path)
                .map(|b| b.dirty)
                .unwrap_or(false);

            let norm_rel = normalize_rel_path(&rel_path.to_string_lossy());

            ui.horizontal(|ui| {
                let dirty_mark = if dirty { " *" } else { "" };
                let main_badge = if is_main { " ★" } else { "" };
                let label_text = format!("{} {}{}{}", kind.icon(), name, main_badge, dirty_mark);
                let mut rich = RichText::new(label_text).size(12.5).color(if is_active {
                    palette.text_primary
                } else {
                    palette.text_muted
                });
                if is_active {
                    rich = rich.strong();
                }
                let resp = ui
                    .selectable_label(is_active, rich)
                    .on_hover_text(abs_path.display().to_string());
                if resp.clicked() {
                    *clicked_path = Some(rel_path.clone());
                }

                // Show colored badges for remote peers currently viewing/editing this file
                for peer in remote_presences {
                    if peer.active_path.as_deref() == Some(norm_rel.as_str()) {
                        let c = Color32::from_rgb(
                            peer.color_rgb[0],
                            peer.color_rgb[1],
                            peer.color_rgb[2],
                        );
                        ui.label(
                            RichText::new(format!("● {}", peer.name))
                                .size(10.5)
                                .color(c)
                                .strong(),
                        )
                        .on_hover_text(t!(
                            "sidebar.peer_editing_tooltip",
                            name = &peer.name,
                            path = &norm_rel
                        ));
                    }
                }

                resp.context_menu(|ui| {
                    ui.label(
                        RichText::new(abs_path.display().to_string())
                            .monospace()
                            .small(),
                    );
                    ui.separator();
                    if *kind == FileKind::Typst
                        && !is_main
                        && ui.button(t!("sidebar.set_as_main")).clicked()
                    {
                        *new_main_path = Some(rel_path.clone());
                        ui.close_menu();
                    }
                    if ui.button(t!("sidebar.rename_file")).clicked() {
                        *rename_req = Some(rel_path.clone());
                        ui.close_menu();
                    }
                    if !is_main && ui.button(t!("sidebar.delete_file")).clicked() {
                        *delete_req = Some(rel_path.clone());
                        ui.close_menu();
                    }
                });
            });
        }
    }
}
