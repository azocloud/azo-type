//! Collaborative UI window, peer list badges, session controls, and in-editor remote cursor painting.

use std::path::Path;

use egui::text::CCursor;
use egui::text_edit::TextEditOutput;
use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, FontFamily, FontId, Frame, Layout, Margin,
    Pos2, Rect, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2, ViewportBuilder, ViewportClass,
    ViewportId, Window,
};
use rust_i18n::t;

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::normalize_rel_path;
use crate::collab::session::{
    CollabSessionHandle, ConnectedPeerStatus, PeerLifecycleState, UiToSessionCmd,
};
use crate::highlight::ThemePalette;

/// UI state for P2P collaboration held by `AzoTypeApp`.
#[derive(Default)]
pub struct CollabUiState {
    pub show_window: bool,
    pub is_active: bool,
    pub is_host: bool,
    pub local_endpoint_id: String,
    pub host_ticket: Option<String>,
    pub reveal_ticket: bool,
    pub join_ticket_input: String,
    pub local_only_mode: bool,
    pub peers: Vec<ConnectedPeerStatus>,
    pub remote_presences: Vec<PeerPresence>,
    pub last_error: Option<String>,
    pub status_banner: Option<String>,
}

impl CollabUiState {
    pub fn connected_peer_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| p.state == PeerLifecycleState::Connected)
            .count()
    }

    /// Returns all remote peers currently viewing `rel_path`.
    pub fn peers_in_file(&self, rel_path: &Path) -> Vec<&PeerPresence> {
        let target = normalize_rel_path(&rel_path.to_string_lossy());
        self.remote_presences
            .iter()
            .filter(|p| p.active_path.as_deref() == Some(target.as_str()))
            .collect()
    }

    pub fn on_session_started(
        &mut self,
        is_host: bool,
        ticket_str: Option<String>,
        local_endpoint_id: String,
    ) {
        self.is_active = true;
        self.is_host = is_host;
        self.host_ticket = ticket_str;
        self.local_endpoint_id = local_endpoint_id;
        self.last_error = None;
    }

    pub fn reset_session(&mut self) {
        self.is_active = false;
        self.host_ticket = None;
        self.peers.clear();
        self.remote_presences.clear();
    }
}

/// Renders the P2P Collaboration window (`Project > Collaboration (P2P)...`) as a native OS sub-window.
#[allow(clippy::too_many_arguments)]
pub fn render_collaboration_window(
    ctx: &Context,
    dark_mode: bool,
    project_root: &Path,
    user_name: &mut String,
    user_color: &mut [u8; 3],
    state: &mut CollabUiState,
    session_handle: &CollabSessionHandle,
) {
    if !state.show_window {
        return;
    }

    let mut open = state.show_window;
    let mut close_clicked = false;
    let palette = ThemePalette::for_mode(dark_mode);
    let border_col = palette.panel_border;
    let card_bg = palette.editor_canvas;
    let win_bg = palette.chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("azo_type_p2p_collab_os_window");
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("collab.window_title"))
            .with_inner_size([560.0, 500.0])
            .with_min_inner_size([440.0, 360.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            // 1. Non-blocking Error / Status Banners
            if let Some(err) = state.last_error.clone() {
                Frame::new()
                    .fill(if dark_mode {
                        Color32::from_rgb(60, 20, 28)
                    } else {
                        Color32::from_rgb(254, 226, 226)
                    })
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(239, 68, 68)))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("[!] {}", err))
                                    .color(Color32::from_rgb(248, 113, 113))
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button(t!("collab.dismiss_btn")).clicked() {
                                    state.last_error = None;
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }

            if let Some(ref info) = state.status_banner {
                ui.label(
                    RichText::new(info)
                        .small()
                        .color(Color32::from_rgb(56, 189, 248)),
                );
                ui.add_space(4.0);
            }

            // 2. Identity Configuration (Display Name + Cursor Color)
            Frame::new()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(t!("collab.identity_heading"))
                            .strong()
                            .size(11.5),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("collab.display_name_label"));
                        let name_changed = ui
                            .add(
                                TextEdit::singleline(user_name)
                                    .desired_width(190.0)
                                    .hint_text(t!("collab.display_name_hint")),
                            )
                            .changed();

                        ui.add_space(10.0);
                        ui.label(t!("collab.cursor_color_label"));
                        let color_changed = ui.color_edit_button_srgb(user_color).changed();

                        if (name_changed || color_changed) && state.is_active {
                            session_handle.send(UiToSessionCmd::UpdateIdentity {
                                local_name: user_name.clone(),
                                local_color: *user_color,
                            });
                        }
                    });
                });

            ui.add_space(8.0);

            // 3. Session Controls (Host or Join or Active Session Info)
            Frame::new()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    if !state.is_active {
                        ui.label(
                            RichText::new(t!("collab.session_heading"))
                                .strong()
                                .size(11.5),
                        );
                        ui.add_space(4.0);
                        let folder_str = t!(
                            "collab.project_folder",
                            path = project_root.display().to_string()
                        );
                        ui.label(RichText::new(folder_str).small().monospace().weak());
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            let host_btn = egui::Button::new(
                                RichText::new(t!("collab.start_host_btn"))
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(37, 99, 235));

                            if ui.add(host_btn).clicked() {
                                state.last_error = None;
                                state.status_banner = Some(t!("collab.starting_host").to_string());
                                session_handle.send(UiToSessionCmd::StartHosting {
                                    project_root: project_root.to_path_buf(),
                                    local_name: user_name.clone(),
                                    local_color: *user_color,
                                    local_only: state.local_only_mode,
                                });
                            }

                            ui.checkbox(&mut state.local_only_mode, t!("collab.lan_only"));
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);

                        ui.label(RichText::new(t!("collab.join_heading")).strong());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                TextEdit::singleline(&mut state.join_ticket_input)
                                    .desired_width(350.0)
                                    .password(!state.reveal_ticket)
                                    .hint_text(t!("collab.ticket_hint")),
                            );
                            let hide_show = if state.reveal_ticket {
                                t!("collab.hide_ticket")
                            } else {
                                t!("collab.show_ticket")
                            };
                            if ui.small_button(hide_show).clicked() {
                                state.reveal_ticket = !state.reveal_ticket;
                            }
                            if ui
                                .button(RichText::new(t!("collab.join_btn")).strong())
                                .clicked()
                                && !state.join_ticket_input.trim().is_empty()
                            {
                                state.last_error = None;
                                state.status_banner = Some(t!("collab.dialing_host").to_string());
                                session_handle.send(UiToSessionCmd::JoinWithTicket {
                                    project_root: project_root.to_path_buf(),
                                    ticket_str: state.join_ticket_input.trim().to_string(),
                                    local_name: user_name.clone(),
                                    local_color: *user_color,
                                    local_only: state.local_only_mode,
                                });
                            }
                        });
                    } else {
                        // Active Session Header
                        ui.horizontal(|ui| {
                            let role_badge = if state.is_host {
                                t!("collab.active_session_host")
                            } else {
                                t!("collab.active_session_participant")
                            };
                            ui.label(
                                RichText::new(role_badge)
                                    .strong()
                                    .color(Color32::from_rgb(52, 211, 153)),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let end_label = if state.is_host {
                                    t!("collab.end_session_btn")
                                } else {
                                    t!("collab.leave_session_btn")
                                };
                                let leave_btn = egui::Button::new(
                                    RichText::new(end_label).color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(220, 38, 38));
                                if ui.add(leave_btn).clicked() {
                                    session_handle.send(UiToSessionCmd::LeaveSession);
                                }
                            });
                        });

                        let short_ep: String = state.local_endpoint_id.chars().take(16).collect();
                        let node_id_text = t!("collab.local_node_id", id = short_ep);
                        ui.label(RichText::new(node_id_text).small().monospace().weak());

                        if let Some(ref ticket) = state.host_ticket {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(t!("collab.security_notice"))
                                    .small()
                                    .color(Color32::from_rgb(251, 191, 36)),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button(t!("collab.copy_ticket_btn")).clicked() {
                                    ctx.copy_text(ticket.clone());
                                    state.status_banner =
                                        Some(t!("collab.ticket_copied").to_string());
                                }
                                let mask_reveal = if state.reveal_ticket {
                                    t!("collab.mask_ticket")
                                } else {
                                    t!("collab.reveal_ticket")
                                };
                                if ui.button(mask_reveal).clicked() {
                                    state.reveal_ticket = !state.reveal_ticket;
                                }
                                let preview = if state.reveal_ticket {
                                    let head: String = ticket.chars().take(42).collect();
                                    format!("{}...", head)
                                } else {
                                    "typst-collab-v1:••••••••••••••••••••••••".to_string()
                                };
                                ui.label(RichText::new(preview).small().monospace());
                            });
                        }
                    }
                });

            ui.add_space(8.0);

            // 4. Connected Peers & Presence List
            Frame::new()
                .fill(card_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let header_text = t!(
                            "collab.connected_peers_heading",
                            count = state.connected_peer_count()
                        );
                        ui.label(RichText::new(header_text).strong().size(11.5));
                    });
                    ui.separator();

                    ScrollArea::vertical()
                        .id_salt("collab_peers_scroll")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            if state.peers.is_empty() {
                                ui.label(
                                    RichText::new(if state.is_active {
                                        t!("collab.waiting_peers")
                                    } else {
                                        t!("collab.offline_mode")
                                    })
                                    .weak()
                                    .small(),
                                );
                            } else {
                                for peer in &state.peers {
                                    let presence = state
                                        .remote_presences
                                        .iter()
                                        .find(|p| p.endpoint_id == peer.peer_id);
                                    let swatch = presence
                                        .map(|p| {
                                            Color32::from_rgb(
                                                p.color_rgb[0],
                                                p.color_rgb[1],
                                                p.color_rgb[2],
                                            )
                                        })
                                        .unwrap_or(Color32::from_rgb(56, 189, 248));

                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(10.0, 10.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(rect.center(), 5.0, swatch);

                                        let name = presence
                                            .map(|p| p.name.as_str())
                                            .unwrap_or(&peer.display_name);
                                        ui.label(RichText::new(name).strong());

                                        let short_id: String =
                                            peer.peer_id.chars().take(10).collect();
                                        ui.label(
                                            RichText::new(format!("({}...)", short_id))
                                                .small()
                                                .monospace()
                                                .weak(),
                                        );

                                        ui.label(RichText::new(peer.state.label()).small().color(
                                            match peer.state {
                                                PeerLifecycleState::Connected => {
                                                    Color32::from_rgb(52, 211, 153)
                                                }
                                                PeerLifecycleState::Disconnected => {
                                                    Color32::from_rgb(248, 113, 113)
                                                }
                                                _ => Color32::from_rgb(251, 191, 36),
                                            },
                                        ));

                                        // Direct vs Relay connection type badge
                                        let route_col = if peer.route.is_direct() {
                                            Color32::from_rgb(52, 211, 153)
                                        } else {
                                            Color32::from_rgb(56, 189, 248)
                                        };
                                        ui.label(
                                            RichText::new(format!(
                                                "[{}]",
                                                peer.route.badge_label()
                                            ))
                                            .small()
                                            .monospace()
                                            .color(route_col),
                                        );

                                        if let Some(active_file) =
                                            presence.and_then(|p| p.active_path.as_deref())
                                        {
                                            let editing_text =
                                                t!("collab.editing_file", path = active_file);
                                            ui.label(RichText::new(editing_text).small().italics());
                                        }
                                    });
                                    ui.add_space(3.0);
                                }
                            }
                        });
                });
        };

        if class == ViewportClass::Embedded {
            Window::new(t!("collab.window_title"))
                .open(&mut open)
                .resizable(true)
                .collapsible(false)
                .default_width(540.0)
                .default_height(470.0)
                .show(ctx, |ui| {
                    render_body(ui);
                });
        } else {
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(win_bg)
                        .inner_margin(Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            render_body(ui);
                        });
                });
            if ctx.input(|i| i.viewport().close_requested()) {
                close_clicked = true;
            }
        }
    });

    state.show_window = open && !close_clicked;
}

/// Paints remote peer cursors, selections, and floating name tags directly onto the `egui::TextEdit` canvas.
pub fn paint_remote_cursors_in_editor(
    ui: &Ui,
    output: &TextEditOutput,
    remote_peers_in_file: &[&PeerPresence],
) {
    if remote_peers_in_file.is_empty() {
        return;
    }

    let painter = ui.painter();
    let galley = &output.galley;
    let galley_pos = output.galley_pos;

    for peer in remote_peers_in_file {
        let Some(caret_char) = peer.cursor_char else {
            continue;
        };

        let peer_color = Color32::from_rgb(peer.color_rgb[0], peer.color_rgb[1], peer.color_rgb[2]);

        // 1. Draw translucent selection highlight if anchor != caret
        if let Some(anchor_char) = peer.anchor_char {
            if anchor_char != caret_char {
                let start_c = anchor_char.min(caret_char);
                let end_c = anchor_char.max(caret_char);
                let r_start = galley
                    .pos_from_cursor(&galley.from_ccursor(CCursor::new(start_c)))
                    .translate(galley_pos.to_vec2());
                let r_end = galley
                    .pos_from_cursor(&galley.from_ccursor(CCursor::new(end_c)))
                    .translate(galley_pos.to_vec2());

                let sel_fill = Color32::from_rgba_unmultiplied(
                    peer.color_rgb[0],
                    peer.color_rgb[1],
                    peer.color_rgb[2],
                    55,
                );

                if (r_start.min.y - r_end.min.y).abs() < 2.0 {
                    // Single-line selection
                    let sel_rect = Rect::from_min_max(
                        Pos2::new(r_start.min.x, r_start.min.y),
                        Pos2::new(r_end.min.x.max(r_start.min.x + 2.0), r_end.max.y),
                    );
                    painter.rect_filled(sel_rect, CornerRadius::same(2), sel_fill);
                } else {
                    // Multi-line selection approximation
                    let sel_rect = Rect::from_min_max(
                        Pos2::new(r_start.min.x.min(r_end.min.x), r_start.min.y),
                        Pos2::new(r_start.max.x.max(r_end.max.x + 40.0), r_end.max.y),
                    );
                    painter.rect_filled(sel_rect, CornerRadius::same(2), sel_fill);
                }
            }
        }

        // 2. Draw vertical colored caret bar
        let caret_local = galley.pos_from_cursor(&galley.from_ccursor(CCursor::new(caret_char)));
        let caret_rect = caret_local.translate(galley_pos.to_vec2());
        let top = Pos2::new(caret_rect.min.x, caret_rect.min.y);
        let bottom = Pos2::new(caret_rect.min.x, caret_rect.max.y);
        painter.line_segment([top, bottom], Stroke::new(2.0_f32, peer_color));

        // 3. Draw peer name tag pill right above the caret
        let tag_font = FontId::new(10.0, FontFamily::Proportional);
        let tag_galley = painter.layout_no_wrap(peer.name.clone(), tag_font, Color32::BLACK);
        let tag_size = tag_galley.size() + Vec2::new(8.0, 3.0);
        let tag_min = Pos2::new(top.x, (top.y - tag_size.y).max(galley_pos.y));
        let tag_rect = Rect::from_min_size(tag_min, tag_size);

        painter.rect_filled(tag_rect, CornerRadius::same(3), peer_color);
        painter.galley(
            tag_rect.min + Vec2::new(4.0, 1.5),
            tag_galley,
            Color32::BLACK,
        );
    }
}
