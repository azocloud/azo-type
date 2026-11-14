//! Collaborative UI window, peer list badges, session controls, and in-editor remote cursor painting.

use std::path::Path;

use egui::{
    Align, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText,
    ScrollArea, Stroke, TextEdit, Ui, Vec2, ViewportBuilder, ViewportClass, ViewportId, Window,
};
use rust_i18n::t;

use crate::collab::awareness::PeerPresence;
pub use crate::collab::cursor_overlay::paint_remote_cursors_in_editor;
use crate::collab::doc::normalize_rel_path;
use crate::collab::session::{ConnectedPeerStatus, PeerLifecycleState};
use crate::highlight::ThemePalette;

/// User intentions emitted by the P2P Collaboration window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabWindowAction {
    StartHosting {
        local_name: String,
        local_color: [u8; 3],
        local_only: bool,
    },
    JoinWithTicket {
        ticket_str: String,
        local_name: String,
        local_color: [u8; 3],
        local_only: bool,
    },
    UpdateIdentity {
        local_name: String,
        local_color: [u8; 3],
    },
    LeaveSession,
    CopyTicket(String),
}

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

    pub fn dismiss_error(&mut self) {
        self.last_error = None;
    }

    pub fn toggle_reveal_ticket(&mut self) {
        self.reveal_ticket = !self.reveal_ticket;
    }

    pub fn masked_ticket_preview(&self, ticket: &str) -> String {
        if self.reveal_ticket {
            let head: String = ticket.chars().take(42).collect();
            format!("{head}...")
        } else {
            "typst-collab-v1:••••••••••••••••••••••••".to_string()
        }
    }

    pub fn prepare_start_hosting(
        &mut self,
        local_name: &str,
        local_color: [u8; 3],
    ) -> CollabWindowAction {
        self.last_error = None;
        self.status_banner = Some(t!("collab.starting_host").to_string());
        CollabWindowAction::StartHosting {
            local_name: local_name.to_string(),
            local_color,
            local_only: self.local_only_mode,
        }
    }

    pub fn prepare_join_with_ticket(
        &mut self,
        local_name: &str,
        local_color: [u8; 3],
    ) -> Option<CollabWindowAction> {
        let trimmed = self.join_ticket_input.trim();
        if trimmed.is_empty() {
            return None;
        }
        self.last_error = None;
        self.status_banner = Some(t!("collab.dialing_host").to_string());
        Some(CollabWindowAction::JoinWithTicket {
            ticket_str: trimmed.to_string(),
            local_name: local_name.to_string(),
            local_color,
            local_only: self.local_only_mode,
        })
    }

    pub fn prepare_copy_ticket(&mut self) -> Option<CollabWindowAction> {
        let ticket = self.host_ticket.clone()?;
        self.status_banner = Some(t!("collab.ticket_copied").to_string());
        Some(CollabWindowAction::CopyTicket(ticket))
    }
}

/// Renders the P2P Collaboration window (`Project > Collaboration (P2P)...`) as a native OS sub-window.
pub fn render_collaboration_window(
    ctx: &Context,
    dark_mode: bool,
    project_root: &Path,
    user_name: &mut String,
    user_color: &mut [u8; 3],
    state: &mut CollabUiState,
) -> Vec<CollabWindowAction> {
    if !state.show_window {
        return Vec::new();
    }

    let mut open = state.show_window;
    let mut close_clicked = false;
    let mut actions = Vec::new();
    let win_bg = ThemePalette::for_mode(dark_mode).chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("azo_type_p2p_collab_os_window");
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("collab.window_title"))
            .with_inner_size([560.0, 500.0])
            .with_min_inner_size([440.0, 360.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        if class == ViewportClass::Embedded {
            Window::new(t!("collab.window_title"))
                .open(&mut open)
                .resizable(true)
                .collapsible(false)
                .default_width(540.0)
                .default_height(470.0)
                .show(ctx, |ui| {
                    actions.extend(render_collaboration_body(
                        ui,
                        dark_mode,
                        project_root,
                        user_name,
                        user_color,
                        state,
                    ));
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
                            actions.extend(render_collaboration_body(
                                ui,
                                dark_mode,
                                project_root,
                                user_name,
                                user_color,
                                state,
                            ));
                        });
                });
            if ctx.input(|i| i.viewport().close_requested()) {
                close_clicked = true;
            }
        }
    });

    state.show_window = open && !close_clicked;
    actions
}

/// Renders the inner body of the P2P Collaboration dialog and returns emitted user actions.
pub fn render_collaboration_body(
    ui: &mut Ui,
    dark_mode: bool,
    project_root: &Path,
    user_name: &mut String,
    user_color: &mut [u8; 3],
    state: &mut CollabUiState,
) -> Vec<CollabWindowAction> {
    let mut actions = Vec::new();
    let palette = ThemePalette::for_mode(dark_mode);
    let border_col = palette.panel_border;
    let card_bg = palette.editor_canvas;

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
                            state.dismiss_error();
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
                let name_label = ui.label(t!("collab.display_name_label"));
                let name_changed = ui
                    .add(
                        TextEdit::singleline(user_name)
                            .desired_width(190.0)
                            .hint_text(t!("collab.display_name_hint")),
                    )
                    .labelled_by(name_label.id)
                    .changed();

                ui.add_space(10.0);
                ui.label(t!("collab.cursor_color_label"));
                let color_changed = ui.color_edit_button_srgb(user_color).changed();

                if (name_changed || color_changed) && state.is_active {
                    actions.push(CollabWindowAction::UpdateIdentity {
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
                        actions.push(state.prepare_start_hosting(user_name, *user_color));
                    }

                    ui.checkbox(&mut state.local_only_mode, t!("collab.lan_only"));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                let join_heading = ui.label(RichText::new(t!("collab.join_heading")).strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut state.join_ticket_input)
                            .desired_width(350.0)
                            .password(!state.reveal_ticket)
                            .hint_text(t!("collab.ticket_hint")),
                    )
                    .labelled_by(join_heading.id);
                    let hide_show = if state.reveal_ticket {
                        t!("collab.hide_ticket")
                    } else {
                        t!("collab.show_ticket")
                    };
                    if ui.small_button(hide_show).clicked() {
                        state.toggle_reveal_ticket();
                    }
                    if ui
                        .button(RichText::new(t!("collab.join_btn")).strong())
                        .clicked()
                    {
                        if let Some(act) = state.prepare_join_with_ticket(user_name, *user_color) {
                            actions.push(act);
                        }
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
                        let leave_btn =
                            egui::Button::new(RichText::new(end_label).color(Color32::WHITE))
                                .fill(Color32::from_rgb(220, 38, 38));
                        if ui.add(leave_btn).clicked() {
                            actions.push(CollabWindowAction::LeaveSession);
                        }
                    });
                });

                let short_ep: String = state.local_endpoint_id.chars().take(16).collect();
                let node_id_text = t!("collab.local_node_id", id = short_ep);
                ui.label(RichText::new(node_id_text).small().monospace().weak());

                if state.host_ticket.is_some() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(t!("collab.security_notice"))
                            .small()
                            .color(Color32::from_rgb(251, 191, 36)),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(t!("collab.copy_ticket_btn")).clicked() {
                            if let Some(act) = state.prepare_copy_ticket() {
                                actions.push(act);
                            }
                        }
                        let mask_reveal = if state.reveal_ticket {
                            t!("collab.mask_ticket")
                        } else {
                            t!("collab.reveal_ticket")
                        };
                        if ui.button(mask_reveal).clicked() {
                            state.toggle_reveal_ticket();
                        }
                        let preview = state
                            .host_ticket
                            .as_deref()
                            .map(|t| state.masked_ticket_preview(t))
                            .unwrap_or_default();
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

                                let short_id: String = peer.peer_id.chars().take(10).collect();
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

                                let route_col = if peer.route.is_direct() {
                                    Color32::from_rgb(52, 211, 153)
                                } else {
                                    Color32::from_rgb(56, 189, 248)
                                };
                                ui.label(
                                    RichText::new(format!("[{}]", peer.route.badge_label()))
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

    actions
}

#[cfg(test)]
#[path = "../tests/collab/ui_tests.rs"]
mod tests;
