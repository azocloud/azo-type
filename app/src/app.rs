use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{
    Align, CollapsingHeader, Color32, Context, CornerRadius, FontFamily, FontId, Frame, Key,
    Layout, Margin, Modifiers, Pos2, Rect, RichText, ScrollArea, Sense, SidePanel, Slider, Stroke,
    StrokeKind, TextEdit, TopBottomPanel, Ui, Vec2, Visuals, Window,
};

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::{normalize_rel_path, CharSelection};
use crate::collab::session::{CollabSessionHandle, SessionToUiEvent, UiToSessionCmd};
use crate::collab::ui::{
    paint_remote_cursors_in_editor, render_collaboration_window, CollabUiState,
};
use crate::compiler::{
    compile_project, export_pdf_bytes_with_config, upload_page_textures, CompileSnapshot,
    DiagnosticItem, PdfArchivalStandard, PdfExportConfig, PdfVersionChoice, RenderedPageData,
    RenderedPageTexture,
};
use crate::highlight::highlight_typst_code;
use crate::project::{ensure_sample_project_on_disk, DiskProject, FileKind, FsTreeNode};
use crate::world::SharedWorldResources;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    NewFile,
    OpenFolder,
    RescanFolder,
    SaveFile,
    SaveAll,
    ExportPdf,
    OpenSettings,
    OpenCollab,
    ToggleSidebar,
    ToggleTheme,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CompileNow,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    EditorAndDisk,
    CompilerAndPreview,
}

#[derive(Debug, Clone)]
pub struct IdeSettings {
    pub dark_mode: bool,
    pub editor_font_size: f32,
    pub show_line_numbers: bool,
    pub syntax_highlighting: bool,
    pub auto_save_on_edit: bool,
    pub show_hidden_files: bool,
    pub auto_compile: bool,
    pub debounce_ms: u64,
    pub render_dpi_scale: f32,
    pub user_name: String,
    pub user_color: [u8; 3],
}

impl Default for IdeSettings {
    fn default() -> Self {
        let default_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "Author".to_string());
        Self {
            dark_mode: true,
            editor_font_size: 13.5,
            show_line_numbers: true,
            syntax_highlighting: true,
            auto_save_on_edit: true,
            show_hidden_files: false,
            auto_compile: true,
            debounce_ms: 120,
            render_dpi_scale: 2.0,
            user_name: default_user,
            user_color: [56, 189, 248],
        }
    }
}

pub enum IoEvent {
    FolderPicked(PathBuf),
    StatusMessage(String),
}

pub struct TypstIdeApp {
    pub resources: Arc<SharedWorldResources>,
    pub project: DiskProject,
    pub settings: IdeSettings,
    pub show_sidebar: bool,
    pub zoom: f32,

    // Settings sub-window state (opened via File > Settings)
    pub show_settings_window: bool,
    pub settings_tab: SettingsTab,

    // Export PDF dialog state (opened via Export PDF button or File > Export PDF...)
    pub show_export_pdf_dialog: bool,
    pub pdf_export_config: PdfExportConfig,
    pub export_pdf_error: Option<String>,

    // P2P Collaboration state & background actor handle
    pub collab_ui: CollabUiState,
    pub collab_session: CollabSessionHandle,
    pub pending_cursor_restore: Option<CharSelection>,
    pub last_sent_cursor: Option<(PathBuf, CharSelection)>,

    // Compilation state
    pub last_compiled_revision: u64,
    pub last_compiled_dpi: f32,
    pub pending_compile: bool,
    pub last_edit_time: f64,
    pub compile_in_progress: bool,
    pub compile_duration_ms: f64,
    pub page_data: Vec<RenderedPageData>,
    pub page_textures: Vec<RenderedPageTexture>,
    pub diagnostics: Vec<DiagnosticItem>,

    // New file creation & rename modal/inline state
    pub show_new_file_input: bool,
    pub new_file_name: String,
    pub rename_target_file: Option<PathBuf>,
    pub rename_file_input: String,
    pub inspect_asset_mode: bool,

    // Status bar notification
    pub status_notice: String,

    // Channels for native file dialogs & background compilation
    pub io_tx: Sender<IoEvent>,
    pub io_rx: Receiver<IoEvent>,
    pub compile_tx: Sender<CompileSnapshot>,
    pub compile_rx: Receiver<CompileSnapshot>,

    // Native macOS OS Menu Bar handle
    #[cfg(target_os = "macos")]
    pub macos_menu: Option<macos_native_menu::MacOsMenuBar>,

    // Screenshot automation
    pub screenshot_path: Option<PathBuf>,
    pub frame_count: u32,
    pub screenshot_requested: bool,
}

impl TypstIdeApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_folder: Option<PathBuf>,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        let resources = Arc::new(SharedWorldResources::new());
        let settings = IdeSettings::default();

        let target_folder = initial_folder.unwrap_or_else(ensure_sample_project_on_disk);
        let project = DiskProject::open_folder(&target_folder, settings.show_hidden_files)
            .unwrap_or_else(|_| {
                let fallback = ensure_sample_project_on_disk();
                DiskProject::open_folder(&fallback, false).expect("Failed to open sample project")
            });

        Self::apply_theme(&cc.egui_ctx, settings.dark_mode);

        let initial_snapshot = compile_project(
            Arc::clone(&resources),
            &project,
            settings.render_dpi_scale,
        );
        let page_textures = upload_page_textures(&cc.egui_ctx, &initial_snapshot.pages);

        let (io_tx, io_rx) = mpsc::channel();
        let (compile_tx, compile_rx) = mpsc::channel();
        let collab_session = CollabSessionHandle::spawn(Some(cc.egui_ctx.clone()));

        let status_notice = format!(
            "Opened disk project '{}' ({})",
            project.project_name,
            project.root_dir.display()
        );

        #[cfg(target_os = "macos")]
        let macos_menu = macos_native_menu::MacOsMenuBar::init();

        Self {
            resources,
            last_compiled_revision: initial_snapshot.revision,
            last_compiled_dpi: settings.render_dpi_scale,
            pending_compile: false,
            last_edit_time: 0.0,
            compile_in_progress: false,
            compile_duration_ms: initial_snapshot.duration_ms,
            page_data: initial_snapshot.pages,
            page_textures,
            diagnostics: initial_snapshot.diagnostics,
            project,
            settings,
            show_sidebar: true,
            zoom: 1.0,
            show_settings_window: false,
            settings_tab: SettingsTab::Appearance,
            show_export_pdf_dialog: false,
            pdf_export_config: PdfExportConfig::default(),
            export_pdf_error: None,
            collab_ui: CollabUiState::default(),
            collab_session,
            pending_cursor_restore: None,
            last_sent_cursor: None,
            show_new_file_input: false,
            new_file_name: "chapters/appendix.typ".to_string(),
            rename_target_file: None,
            rename_file_input: String::new(),
            inspect_asset_mode: false,
            status_notice,
            io_tx,
            io_rx,
            compile_tx,
            compile_rx,
            #[cfg(target_os = "macos")]
            macos_menu,
            screenshot_path,
            frame_count: 0,
            screenshot_requested: false,
        }
    }

    pub fn apply_theme(ctx: &Context, dark_mode: bool) {
        let mut visuals = if dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        if dark_mode {
            visuals.panel_fill = Color32::from_rgb(17, 24, 39);
            visuals.window_fill = Color32::from_rgb(24, 33, 50);
            visuals.extreme_bg_color = Color32::from_rgb(11, 15, 25);
            visuals.faint_bg_color = Color32::from_rgb(24, 33, 47);
            visuals.code_bg_color = Color32::from_rgb(15, 23, 42);
            visuals.selection.bg_fill = Color32::from_rgb(37, 99, 235);
            visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254));
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(24, 33, 50);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 72));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 41, 59);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(51, 65, 85);
            visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        } else {
            visuals.panel_fill = Color32::from_rgb(248, 250, 252);
            visuals.window_fill = Color32::from_rgb(255, 255, 255);
            visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
            visuals.faint_bg_color = Color32::from_rgb(241, 245, 249);
            visuals.code_bg_color = Color32::from_rgb(241, 245, 249);
            visuals.selection.bg_fill = Color32::from_rgb(59, 130, 246);
            visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(255, 255, 255));
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(241, 245, 249);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(226, 232, 240);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(203, 213, 225);
            visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        }

        ctx.set_visuals(visuals);
    }

    fn dispatch_menu_action(&mut self, ctx: &Context, action: MenuAction) {
        match action {
            MenuAction::NewFile => {
                self.show_new_file_input = true;
            }
            MenuAction::OpenFolder => {
                self.open_folder_dialog(ctx);
            }
            MenuAction::RescanFolder => {
                if let Err(e) = self.project.rescan_tree() {
                    self.status_notice = format!("Rescan failed: {}", e);
                } else {
                    self.status_notice =
                        format!("Rescanned disk folder '{}'", self.project.root_dir.display());
                    self.trigger_compile_now(ctx);
                }
            }
            MenuAction::SaveFile => match self.project.save_active_file() {
                Ok(path) => {
                    self.status_notice = format!("Saved to disk: {}", path.display());
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice = format!("Save error: {}", e);
                }
            },
            MenuAction::SaveAll => match self.project.save_all_files() {
                Ok(n) => {
                    self.status_notice = format!(
                        "Saved {} modified file(s) in {}",
                        n,
                        self.project.root_dir.display()
                    );
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice = format!("Save all error: {}", e);
                }
            },
            MenuAction::ExportPdf => {
                self.show_export_pdf_dialog = true;
                self.export_pdf_error = None;
            }
            MenuAction::OpenSettings => {
                self.show_settings_window = true;
            }
            MenuAction::OpenCollab => {
                self.collab_ui.show_window = true;
            }
            MenuAction::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
            }
            MenuAction::ToggleTheme => {
                self.settings.dark_mode = !self.settings.dark_mode;
                Self::apply_theme(ctx, self.settings.dark_mode);
            }
            MenuAction::ZoomIn => {
                self.zoom = (self.zoom + 0.15).clamp(0.4, 2.5);
            }
            MenuAction::ZoomOut => {
                self.zoom = (self.zoom - 0.15).clamp(0.4, 2.5);
            }
            MenuAction::ZoomReset => {
                self.zoom = 1.0;
            }
            MenuAction::CompileNow => {
                self.trigger_compile_now(ctx);
            }
            MenuAction::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn trigger_compile_now(&mut self, ctx: &Context) {
        self.pending_compile = false;
        if !self.compile_in_progress {
            self.compile_in_progress = true;
            let resources = Arc::clone(&self.resources);
            let project_clone = self.project.clone();
            let dpi = self.settings.render_dpi_scale;
            let tx = self.compile_tx.clone();
            let ctx_clone = ctx.clone();

            std::thread::spawn(move || {
                let snapshot = compile_project(resources, &project_clone, dpi);
                let _ = tx.send(snapshot);
                ctx_clone.request_repaint();
            });
        } else {
            self.pending_compile = true;
        }
    }

    fn apply_compile_snapshot(&mut self, ctx: &Context, snapshot: CompileSnapshot) {
        self.compile_in_progress = false;
        self.last_compiled_revision = snapshot.revision;
        self.last_compiled_dpi = snapshot.pixel_per_pt;
        self.compile_duration_ms = snapshot.duration_ms;
        self.diagnostics = snapshot.diagnostics;

        if !snapshot.pages.is_empty() {
            self.page_textures = upload_page_textures(ctx, &snapshot.pages);
            self.page_data = snapshot.pages;
        }
    }

    fn poll_channels(&mut self, ctx: &Context) {
        #[cfg(target_os = "macos")]
        if let Some(ref menu) = self.macos_menu {
            while let Some(action) = menu.poll_action() {
                self.dispatch_menu_action(ctx, action);
            }
        }

        while let Ok(snapshot) = self.compile_rx.try_recv() {
            self.apply_compile_snapshot(ctx, snapshot);
        }

        while let Ok(event) = self.io_rx.try_recv() {
            match event {
                IoEvent::FolderPicked(path) => {
                    if self.collab_ui.is_active {
                        self.collab_session.send(UiToSessionCmd::LeaveSession);
                    }
                    match DiskProject::open_folder(&path, self.settings.show_hidden_files) {
                        Ok(proj) => {
                            let name = proj.project_name.clone();
                            let root_str = proj.root_dir.display().to_string();
                            self.project = proj;
                            self.status_notice =
                                format!("Opened project '{}' ({})", name, root_str);
                            self.trigger_compile_now(ctx);
                        }
                        Err(err) => {
                            self.status_notice = format!("Could not open folder: {}", err);
                        }
                    }
                }
                IoEvent::StatusMessage(msg) => {
                    self.status_notice = msg;
                }
            }
        }

        // Poll background P2P Collaboration Actor events
        let mut need_compile = false;
        while let Some(ev) = self.collab_session.try_recv() {
            match ev {
                SessionToUiEvent::SessionStarted {
                    is_host,
                    ticket_str,
                    local_endpoint_id,
                    files,
                } => {
                    self.collab_ui.is_active = true;
                    self.collab_ui.is_host = is_host;
                    self.collab_ui.host_ticket = ticket_str;
                    self.collab_ui.local_endpoint_id = local_endpoint_id;
                    self.collab_ui.last_error = None;
                    if !files.is_empty() {
                        self.project.sync_from_crdt_files(&files);
                        need_compile = true;
                    }
                }
                SessionToUiEvent::SessionStopped { reason } => {
                    self.collab_ui.is_active = false;
                    self.collab_ui.is_host = false;
                    self.collab_ui.host_ticket = None;
                    self.collab_ui.peers.clear();
                    self.collab_ui.remote_presences.clear();
                    self.collab_ui.status_banner = Some(reason.clone());
                    self.status_notice = reason;
                }
                SessionToUiEvent::PeerListChanged { peers } => {
                    self.collab_ui.peers = peers;
                }
                SessionToUiEvent::RemoteFilesUpdated {
                    files,
                    disk_updated_paths: _,
                    preserved_selection,
                } => {
                    self.project.sync_from_crdt_files(&files);
                    if let Some(sel) = preserved_selection {
                        self.pending_cursor_restore = Some(sel);
                    }
                    need_compile = true;
                }
                SessionToUiEvent::RemotePresenceUpdated { presences } => {
                    self.collab_ui.remote_presences = presences;
                }
                SessionToUiEvent::StatusNotice { message } => {
                    self.collab_ui.status_banner = Some(message.clone());
                    self.status_notice = message;
                }
                SessionToUiEvent::CollabError { message } => {
                    self.collab_ui.last_error = Some(message.clone());
                    self.status_notice = format!("Collab error: {}", message);
                }
            }
        }

        if need_compile {
            self.trigger_compile_now(ctx);
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        let mut triggered = None;
        let collab_active = self.collab_ui.is_active;
        let active_rel = self.project.active_file.clone();

        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::Comma) {
                triggered = Some(MenuAction::OpenSettings);
            } else if i.consume_key(Modifiers::COMMAND, Key::S) {
                triggered = Some(MenuAction::SaveFile);
            } else if i.consume_key(Modifiers::COMMAND, Key::O) {
                triggered = Some(MenuAction::OpenFolder);
            } else if i.consume_key(Modifiers::COMMAND, Key::N) {
                triggered = Some(MenuAction::NewFile);
            } else if i.consume_key(Modifiers::COMMAND, Key::B) {
                triggered = Some(MenuAction::ToggleSidebar);
            } else if i.consume_key(Modifiers::COMMAND, Key::E) {
                triggered = Some(MenuAction::ExportPdf);
            } else if i.consume_key(Modifiers::NONE, Key::F5) {
                triggered = Some(MenuAction::RescanFolder);
            } else if collab_active && i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
            {
                self.collab_session
                    .send(UiToSessionCmd::LocalRedo { rel_path: active_rel.clone() });
            } else if collab_active && i.consume_key(Modifiers::COMMAND, Key::Y) {
                self.collab_session
                    .send(UiToSessionCmd::LocalRedo { rel_path: active_rel.clone() });
            } else if collab_active && i.consume_key(Modifiers::COMMAND, Key::Z) {
                self.collab_session
                    .send(UiToSessionCmd::LocalUndo { rel_path: active_rel.clone() });
            }
        });
        if let Some(action) = triggered {
            self.dispatch_menu_action(ctx, action);
        }
    }

    fn open_folder_dialog(&self, ctx: &Context) {
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();
        let start_dir = self.project.root_dir.clone();

        std::thread::spawn(move || {
            let task = async move {
                if let Some(folder) = rfd::AsyncFileDialog::new()
                    .set_title("Open Typst Project Folder")
                    .set_directory(&start_dir)
                    .pick_folder()
                    .await
                {
                    let _ = tx.send(IoEvent::FolderPicked(folder.path().to_path_buf()));
                    ctx_clone.request_repaint();
                }
            };
            pollster::block_on_or_spin(task);
        });
    }

    fn export_pdf_action(&mut self, ctx: &Context) {
        let _ = self.project.save_all_files();
        let resources = Arc::clone(&self.resources);
        let project_snapshot = self.project.clone();
        let config = self.pdf_export_config.clone();
        let entry_file = self.project.effective_compile_entrypoint();
        let default_pdf_name = format!(
            "{}.pdf",
            entry_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document")
        );
        let start_dir = self.project.root_dir.clone();
        let badge = config.summary_badge();

        match export_pdf_bytes_with_config(resources, &project_snapshot, &config) {
            Ok(pdf_bytes) => {
                self.export_pdf_error = None;
                self.show_export_pdf_dialog = false;
                let size_kb = pdf_bytes.len() as f64 / 1024.0;
                self.status_notice = format!(
                    "Compiled {} ({:.1} KB) — Choose destination on disk...",
                    badge, size_kb
                );
                let tx = self.io_tx.clone();
                let ctx_clone = ctx.clone();

                std::thread::spawn(move || {
                    let task = async move {
                        if let Some(handle) = rfd::AsyncFileDialog::new()
                            .set_title("Export Compiled PDF to Disk")
                            .set_directory(&start_dir)
                            .set_file_name(&default_pdf_name)
                            .add_filter("PDF Document", &["pdf"])
                            .save_file()
                            .await
                        {
                            let out_path = handle.path().to_path_buf();
                            match std::fs::write(&out_path, &pdf_bytes) {
                                Ok(()) => {
                                    let _ = tx.send(IoEvent::StatusMessage(format!(
                                        "Exported {} to '{}' ({:.1} KB)",
                                        badge,
                                        out_path.display(),
                                        size_kb
                                    )));
                                }
                                Err(err) => {
                                    let _ = tx.send(IoEvent::StatusMessage(format!(
                                        "PDF write failed: {}",
                                        err
                                    )));
                                }
                            }
                            ctx_clone.request_repaint();
                        }
                    };
                    pollster::block_on_or_spin(task);
                });
            }
            Err(diags) => {
                let first_err = diags
                    .first()
                    .map(|d| d.message.clone())
                    .unwrap_or_else(|| "PDF compilation failed.".to_string());
                self.export_pdf_error = Some(first_err.clone());
                self.diagnostics = diags;
                self.status_notice = format!("Cannot export PDF: {}", first_err);
            }
        }
    }

    /// Renders the Export PDF Dialog window allowing the user to select the PDF version
    /// (1.4, 1.5, 1.6, 1.7, 2.0) and PDF standard(s) (PDF/A-1b, PDF/A-1a, PDF/A-2b, PDF/A-2u,
    /// PDF/A-2a, PDF/A-3b, PDF/A-3u, PDF/A-3a, PDF/A-4, PDF/UA-1).
    fn render_export_pdf_dialog(&mut self, ctx: &Context) {
        if !self.show_export_pdf_dialog {
            return;
        }

        let mut open = self.show_export_pdf_dialog;
        let mut close_clicked = false;
        let mut trigger_export = false;

        let dark_mode = self.settings.dark_mode;
        let card_bg = if dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let border_col = if dark_mode {
            Color32::from_rgb(45, 55, 72)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        let entry_file = self.project.effective_compile_entrypoint();

        Window::new("Export PDF — Version & Standards")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Target Document:").strong());
                    ui.code(entry_file.display().to_string());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.pdf_export_config.summary_badge())
                                .strong()
                                .color(Color32::from_rgb(56, 189, 248)),
                        );
                    });
                });

                ui.add_space(6.0);

                // Section 1: PDF Version Selection (1.4, 1.5, 1.6, 1.7, 2.0)
                Frame::new()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0_f32, border_col))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.label(RichText::new("1. PDF Version").strong().size(13.0));
                        ui.label(
                            RichText::new(
                                "Choose the target PDF specification version for the exported document:",
                            )
                            .small()
                            .weak(),
                        );
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for &ver in PdfVersionChoice::ALL {
                                let selected = self.pdf_export_config.version == ver;
                                if ui
                                    .selectable_label(
                                        selected,
                                        RichText::new(format!("PDF {}", ver.short_label()))
                                            .strong(),
                                    )
                                    .on_hover_text(ver.label())
                                    .clicked()
                                {
                                    self.pdf_export_config.version = ver;
                                    self.export_pdf_error = None;
                                }
                            }
                        });
                    });

                ui.add_space(8.0);

                // Section 2: PDF Standard(s) Selection (PDF/A-2a, PDF/A-3a, etc.)
                Frame::new()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0_f32, border_col))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("2. PDF Archival & Conformance Standard (PDF/A)")
                                .strong()
                                .size(13.0),
                        );
                        ui.label(
                            RichText::new(
                                "Select an ISO 19005 archival standard (such as PDF/A-2a, PDF/A-2b, or PDF/A-3a):",
                            )
                            .small()
                            .weak(),
                        );
                        ui.add_space(6.0);

                        // Quick Preset Buttons for common standards
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Quick Presets:").small());
                            let presets = [
                                PdfArchivalStandard::None,
                                PdfArchivalStandard::A2b,
                                PdfArchivalStandard::A2a,
                                PdfArchivalStandard::A3b,
                                PdfArchivalStandard::A3a,
                                PdfArchivalStandard::A1b,
                                PdfArchivalStandard::A1a,
                            ];
                            for std_choice in presets {
                                let is_sel = self.pdf_export_config.archival_standard == std_choice;
                                if ui
                                    .selectable_label(is_sel, std_choice.code_label())
                                    .on_hover_text(std_choice.label())
                                    .clicked()
                                {
                                    self.pdf_export_config.archival_standard = std_choice;
                                    self.export_pdf_error = None;
                                    if self.pdf_export_config.build_standards().is_err() {
                                        if let Some(rec) = std_choice.recommended_version() {
                                            self.pdf_export_config.version = rec;
                                        }
                                    }
                                }
                            }
                        });

                        ui.add_space(6.0);

                        egui::ComboBox::from_id_salt("pdf_archival_standard_combo")
                            .width(460.0)
                            .selected_text(self.pdf_export_config.archival_standard.label())
                            .show_ui(ui, |ui| {
                                for &std_item in PdfArchivalStandard::ALL {
                                    let is_selected =
                                        self.pdf_export_config.archival_standard == std_item;
                                    if ui
                                        .selectable_label(is_selected, std_item.label())
                                        .clicked()
                                    {
                                        self.pdf_export_config.archival_standard = std_item;
                                        self.export_pdf_error = None;
                                        if self.pdf_export_config.build_standards().is_err() {
                                            if let Some(rec) = std_item.recommended_version() {
                                                self.pdf_export_config.version = rec;
                                            }
                                        }
                                    }
                                }
                            });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        if ui
                            .checkbox(
                                &mut self.pdf_export_config.pdf_ua1,
                                "Also enforce PDF/UA-1 (Universal Accessibility — ISO 14289-1)",
                            )
                            .changed()
                        {
                            self.export_pdf_error = None;
                            if self.pdf_export_config.pdf_ua1 {
                                self.pdf_export_config.tagged = true;
                                if self.pdf_export_config.build_standards().is_err() {
                                    self.pdf_export_config.version = PdfVersionChoice::V1_7;
                                }
                            }
                        }

                        let requires_tagged = self.pdf_export_config.pdf_ua1
                            || matches!(
                                self.pdf_export_config.archival_standard,
                                PdfArchivalStandard::A1a
                                    | PdfArchivalStandard::A2a
                                    | PdfArchivalStandard::A3a
                            );
                        if requires_tagged {
                            self.pdf_export_config.tagged = true;
                        }
                        ui.add_enabled(
                            !requires_tagged,
                            egui::Checkbox::new(
                                &mut self.pdf_export_config.tagged,
                                "Generate Tagged PDF (Document Accessibility Structure Tree)",
                            ),
                        );
                    });

                ui.add_space(8.0);

                // Validation status & CLI preview
                match self.pdf_export_config.build_standards() {
                    Ok(_) => {
                        let mut std_flags =
                            vec![self.pdf_export_config.version.short_label().to_string()];
                        if self.pdf_export_config.archival_standard != PdfArchivalStandard::None {
                            std_flags.push(
                                self.pdf_export_config
                                    .archival_standard
                                    .code_label()
                                    .trim_start_matches("PDF/")
                                    .to_ascii_lowercase(),
                            );
                        }
                        if self.pdf_export_config.pdf_ua1 {
                            std_flags.push("ua-1".to_string());
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new("✓ Valid Standard Configuration")
                                    .color(Color32::from_rgb(52, 211, 153))
                                    .strong()
                                    .small(),
                            );
                            ui.separator();
                            ui.code(format!(
                                "typst compile --pdf-standard {} {}",
                                std_flags.join(","),
                                entry_file.display()
                            ));
                        });
                    }
                    Err(validation_err) => {
                        Frame::new()
                            .fill(if dark_mode {
                                Color32::from_rgb(45, 24, 18)
                            } else {
                                Color32::from_rgb(254, 242, 242)
                            })
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(248, 113, 113)))
                            .corner_radius(CornerRadius::same(5))
                            .inner_margin(Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Incompatible Version & Standard: {}",
                                        validation_err
                                    ))
                                    .color(Color32::from_rgb(248, 113, 113))
                                    .strong()
                                    .small(),
                                );
                                if let Some(rec) =
                                    self.pdf_export_config.archival_standard.recommended_version()
                                {
                                    if ui
                                        .small_button(format!(
                                            "Switch to compatible {}",
                                            rec.label()
                                        ))
                                        .clicked()
                                    {
                                        self.pdf_export_config.version = rec;
                                        self.export_pdf_error = None;
                                    }
                                }
                            });
                    }
                }

                if let Some(ref err_msg) = self.export_pdf_error {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Export Error: {}", err_msg))
                            .color(Color32::from_rgb(248, 113, 113))
                            .strong()
                            .small(),
                    );
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);

                let is_valid = self.pdf_export_config.build_standards().is_ok();
                ui.horizontal(|ui| {
                    if ui.button("Reset Defaults").clicked() {
                        self.pdf_export_config = PdfExportConfig::default();
                        self.export_pdf_error = None;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let export_btn = egui::Button::new(
                            RichText::new("Export PDF to Disk...")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if is_valid {
                            Color32::from_rgb(37, 99, 235)
                        } else {
                            Color32::from_rgb(100, 116, 139)
                        })
                        .corner_radius(CornerRadius::same(5));

                        if ui.add_enabled(is_valid, export_btn).clicked() {
                            trigger_export = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });

        self.show_export_pdf_dialog = open && !close_clicked;
        if trigger_export {
            self.export_pdf_action(ctx);
        }
    }

    /// Renders the desktop application menu bar on Linux and Windows.
    /// On macOS, the menu bar is part of the OS at the top of the screen via `muda` and is NOT rendered in-window.
    #[cfg(not(target_os = "macos"))]
    fn render_desktop_menu_bar(&mut self, ctx: &Context) {
        let menu_bg = if self.settings.dark_mode {
            Color32::from_rgb(11, 15, 25)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        let mut pending_action: Option<MenuAction> = None;
        let mut snippet_to_insert: Option<&'static str> = None;

        TopBottomPanel::top("desktop_os_menu_bar")
            .frame(
                Frame::new()
                    .fill(menu_bg)
                    .inner_margin(Margin::symmetric(10, 4))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // FILE MENU
                    ui.menu_button(RichText::new("File").strong(), |ui| {
                        ui.set_min_width(220.0);
                        if ui.button("New File in Project...  (Ctrl+N)").clicked() {
                            pending_action = Some(MenuAction::NewFile);
                            ui.close_menu();
                        }
                        if ui.button("Open Project Folder...  (Ctrl+O)").clicked() {
                            pending_action = Some(MenuAction::OpenFolder);
                            ui.close_menu();
                        }
                        if ui.button("Rescan Folder on Disk  (F5)").clicked() {
                            pending_action = Some(MenuAction::RescanFolder);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Save Active File  (Ctrl+S)").clicked() {
                            pending_action = Some(MenuAction::SaveFile);
                            ui.close_menu();
                        }
                        if ui.button("Save All Project Files").clicked() {
                            pending_action = Some(MenuAction::SaveAll);
                            ui.close_menu();
                        }
                        if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                            pending_action = Some(MenuAction::ExportPdf);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Settings...  (Ctrl+,)").clicked() {
                            pending_action = Some(MenuAction::OpenSettings);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            pending_action = Some(MenuAction::Quit);
                            ui.close_menu();
                        }
                    });

                    // EDIT MENU
                    ui.menu_button(RichText::new("Edit").strong(), |ui| {
                        ui.set_min_width(200.0);
                        if ui.button("Insert Section Heading").clicked() {
                            snippet_to_insert = Some("\n\n== New Section\n");
                            ui.close_menu();
                        }
                        if ui.button("Insert Display Math ($ ... $)").clicked() {
                            snippet_to_insert =
                                Some("\n$ f(x) = integral_0^x e^(-t^2) dif t $\n");
                            ui.close_menu();
                        }
                        if ui.button("Insert Formatted Table").clicked() {
                            snippet_to_insert = Some(
                                "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
                            );
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Settings...  (Ctrl+,)").clicked() {
                            pending_action = Some(MenuAction::OpenSettings);
                            ui.close_menu();
                        }
                    });

                    // VIEW MENU
                    ui.menu_button(RichText::new("View").strong(), |ui| {
                        ui.set_min_width(210.0);
                        let sidebar_txt = if self.show_sidebar {
                            "Hide Project File Tree  (Ctrl+B)"
                        } else {
                            "Show Project File Tree  (Ctrl+B)"
                        };
                        if ui.button(sidebar_txt).clicked() {
                            pending_action = Some(MenuAction::ToggleSidebar);
                            ui.close_menu();
                        }
                        let theme_txt = if self.settings.dark_mode {
                            "Switch to Light Theme"
                        } else {
                            "Switch to Dark Theme"
                        };
                        if ui.button(theme_txt).clicked() {
                            pending_action = Some(MenuAction::ToggleTheme);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Zoom In Preview (+)").clicked() {
                            pending_action = Some(MenuAction::ZoomIn);
                            ui.close_menu();
                        }
                        if ui.button("Zoom Out Preview (-)").clicked() {
                            pending_action = Some(MenuAction::ZoomOut);
                            ui.close_menu();
                        }
                        if ui.button("Reset Preview Zoom (100%)").clicked() {
                            pending_action = Some(MenuAction::ZoomReset);
                            ui.close_menu();
                        }
                    });

                    // PROJECT MENU
                    ui.menu_button(RichText::new("Project").strong(), |ui| {
                        ui.set_min_width(220.0);
                        if ui.button("Collaboration (P2P)...").clicked() {
                            pending_action = Some(MenuAction::OpenCollab);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Compile Document Now").clicked() {
                            pending_action = Some(MenuAction::CompileNow);
                            ui.close_menu();
                        }
                        if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                            pending_action = Some(MenuAction::ExportPdf);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .button("Set Active File as Main Entrypoint")
                            .clicked()
                        {
                            let active = self.project.active_file.clone();
                            if FileKind::from_path(&active) == FileKind::Typst {
                                self.project.set_main_file(active);
                                pending_action = Some(MenuAction::CompileNow);
                            }
                            ui.close_menu();
                        }
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Folder: {}",
                                self.project.root_dir.display()
                            ))
                            .small()
                            .monospace()
                            .weak(),
                        );
                    });
                });
            });

        if let Some(snippet) = snippet_to_insert {
            let auto_save = self.settings.auto_save_on_edit;
            let active_rel = self.project.active_file.clone();
            let mut updated_text = None;
            if let Some(buf) = self.project.active_buffer_mut() {
                if let Some(ref mut text) = buf.text {
                    text.push_str(snippet);
                    updated_text = Some(text.clone());
                    self.project.mark_active_modified(auto_save);
                    self.pending_compile = true;
                    self.last_edit_time = ctx.input(|i| i.time);
                }
            }
            if let (true, Some(new_text)) = (self.collab_ui.is_active, updated_text) {
                self.collab_session.send(UiToSessionCmd::LocalTextEdited {
                    rel_path: active_rel,
                    new_text,
                });
            }
        }

        if let Some(act) = pending_action {
            self.dispatch_menu_action(ctx, act);
        }
    }

    fn render_top_toolbar(&mut self, ctx: &Context) {
        let bar_bg = if self.settings.dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        let mut pending_action: Option<MenuAction> = None;

        TopBottomPanel::top("top_action_toolbar")
            .frame(
                Frame::new()
                    .fill(bar_bg)
                    .inner_margin(Margin::symmetric(12, 7))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    let sidebar_label = if self.show_sidebar {
                        "Hide Tree"
                    } else {
                        "Show Tree"
                    };
                    if ui
                        .selectable_label(
                            self.show_sidebar,
                            RichText::new(sidebar_label).strong(),
                        )
                        .on_hover_text("Toggle Left Project File Tree (Ctrl+B)")
                        .clicked()
                    {
                        pending_action = Some(MenuAction::ToggleSidebar);
                    }

                    ui.separator();

                    ui.label(
                        RichText::new("TYPST STUDIO")
                            .strong()
                            .size(13.5)
                            .color(if self.settings.dark_mode {
                                Color32::from_rgb(56, 189, 248)
                            } else {
                                Color32::from_rgb(2, 132, 199)
                            }),
                    );

                    ui.separator();

                    if ui
                        .button("Open Project Folder...")
                        .on_hover_text("Open a Typst project directory from disk (Ctrl+O)")
                        .clicked()
                    {
                        pending_action = Some(MenuAction::OpenFolder);
                    }

                    if ui
                        .button("+ New File")
                        .on_hover_text("Create a new file on disk inside this project (Ctrl+N)")
                        .clicked()
                    {
                        pending_action = Some(MenuAction::NewFile);
                    }

                    if ui
                        .button("Save")
                        .on_hover_text("Write active buffer to disk (Ctrl+S)")
                        .clicked()
                    {
                        pending_action = Some(MenuAction::SaveFile);
                    }

                    let collab_label = if self.collab_ui.is_active {
                        format!("P2P Live ({})", self.collab_ui.connected_peer_count() + 1)
                    } else {
                        "P2P Collab".to_string()
                    };
                    let collab_text = if self.collab_ui.is_active {
                        RichText::new(collab_label)
                            .strong()
                            .color(Color32::from_rgb(52, 211, 153))
                    } else {
                        RichText::new(collab_label)
                    };
                    if ui
                        .selectable_label(self.collab_ui.show_window, collab_text)
                        .on_hover_text("Real-Time P2P Collaborative Editing (Project > Collaboration)")
                        .clicked()
                    {
                        self.collab_ui.show_window = !self.collab_ui.show_window;
                    }

                    if ui
                        .selectable_label(self.show_settings_window, "Settings")
                        .on_hover_text("Open Settings Window (File > Settings / Ctrl+,)")
                        .clicked()
                    {
                        self.show_settings_window = !self.show_settings_window;
                    }

                    ui.separator();

                    ui.label(RichText::new("Zoom:").small());
                    if ui.small_button("-").clicked() {
                        pending_action = Some(MenuAction::ZoomOut);
                    }
                    if ui
                        .small_button(format!("{:.0}%", self.zoom * 100.0))
                        .on_hover_text("Click to reset zoom to 100%")
                        .clicked()
                    {
                        pending_action = Some(MenuAction::ZoomReset);
                    }
                    if ui.small_button("+").clicked() {
                        pending_action = Some(MenuAction::ZoomIn);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let export_btn = egui::Button::new(
                            RichText::new("Export PDF...")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(37, 99, 235))
                        .corner_radius(CornerRadius::same(5));

                        if ui
                            .add(export_btn)
                            .on_hover_text(
                                "Open Export PDF dialog to choose PDF version (1.4–2.0) and PDF/A standards (Ctrl+E)",
                            )
                            .clicked()
                        {
                            pending_action = Some(MenuAction::ExportPdf);
                        }

                        ui.add_space(4.0);

                        let theme_label = if self.settings.dark_mode {
                            "Light Theme"
                        } else {
                            "Dark Theme"
                        };
                        if ui
                            .button(theme_label)
                            .on_hover_text("Switch between Light and Dark editor themes")
                            .clicked()
                        {
                            pending_action = Some(MenuAction::ToggleTheme);
                        }

                        if ui
                            .button("Compile")
                            .on_hover_text("Force immediate Typst compilation")
                            .clicked()
                        {
                            pending_action = Some(MenuAction::CompileNow);
                        }
                    });
                });

                // Inline New File creation bar on disk
                if self.show_new_file_input {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("New file path on disk:").strong());
                        let resp = ui.add(
                            TextEdit::singleline(&mut self.new_file_name)
                                .desired_width(240.0)
                                .hint_text("e.g. chapters/results.typ"),
                        );
                        let enter_pressed =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if ui.button("Create on Disk").clicked() || enter_pressed {
                            let path_str = self.new_file_name.trim().to_string();
                            match self.project.create_file_on_disk(&path_str) {
                                Ok(rel) => {
                                    let initial_text = self
                                        .project
                                        .read_source_text(&rel)
                                        .unwrap_or_default();
                                    if self.collab_ui.is_active {
                                        self.collab_session.send(
                                            UiToSessionCmd::LocalFileCreated {
                                                rel_path: rel.clone(),
                                                initial_text,
                                            },
                                        );
                                    }
                                    self.status_notice = format!(
                                        "Created '{}' on disk",
                                        self.project.root_dir.join(&rel).display()
                                    );
                                    self.show_new_file_input = false;
                                    self.trigger_compile_now(ctx);
                                }
                                Err(e) => {
                                    self.status_notice = format!("Create file failed: {}", e);
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_new_file_input = false;
                        }
                    });
                }

                // Inline File Rename bar on disk
                if let Some(old_rel) = self.rename_target_file.clone() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Rename '{}' to:", old_rel.display())).strong(),
                        );
                        let resp = ui.add(
                            TextEdit::singleline(&mut self.rename_file_input)
                                .desired_width(240.0),
                        );
                        let enter_pressed =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if ui.button("Rename on Disk").clicked() || enter_pressed {
                            let new_str = self.rename_file_input.trim().to_string();
                            match self.project.rename_file_on_disk(&old_rel, &new_str) {
                                Ok(new_rel) => {
                                    if self.collab_ui.is_active {
                                        self.collab_session.send(
                                            UiToSessionCmd::LocalFileRenamed {
                                                old_rel_path: old_rel.clone(),
                                                new_rel_path: new_rel.clone(),
                                            },
                                        );
                                    }
                                    self.status_notice = format!(
                                        "Renamed '{}' -> '{}'",
                                        old_rel.display(),
                                        new_rel.display()
                                    );
                                    self.rename_target_file = None;
                                    self.trigger_compile_now(ctx);
                                }
                                Err(e) => {
                                    self.status_notice = format!("Rename failed: {}", e);
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.rename_target_file = None;
                        }
                    });
                }
            });

        if let Some(act) = pending_action {
            self.dispatch_menu_action(ctx, act);
        }
    }

    /// Renders the Settings sub-window (`File > Settings`).
    fn render_settings_subwindow(&mut self, ctx: &Context) {
        if !self.show_settings_window {
            return;
        }

        let mut open = self.show_settings_window;
        let mut close_clicked = false;
        let mut theme_changed = false;
        let mut dpi_changed = false;
        let mut rescan_needed = false;

        Window::new("Settings — Typst Studio")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(480.0)
            .default_height(360.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::Appearance,
                        RichText::new("Appearance").strong(),
                    );
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::EditorAndDisk,
                        RichText::new("Editor & Filesystem").strong(),
                    );
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::CompilerAndPreview,
                        RichText::new("Compiler & Preview").strong(),
                    );
                });
                ui.separator();
                ui.add_space(6.0);

                match self.settings_tab {
                    SettingsTab::Appearance => {
                        ui.heading("Theme & Visual Appearance");
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Color Theme:");
                            if ui
                                .radio_value(&mut self.settings.dark_mode, true, "Dark Mode")
                                .changed()
                            {
                                theme_changed = true;
                            }
                            if ui
                                .radio_value(&mut self.settings.dark_mode, false, "Light Mode")
                                .changed()
                            {
                                theme_changed = true;
                            }
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Editor Font Size:");
                            ui.add(
                                Slider::new(&mut self.settings.editor_font_size, 10.0..=22.0)
                                    .suffix(" pt"),
                            );
                        });

                        ui.add_space(8.0);
                        ui.checkbox(
                            &mut self.settings.show_line_numbers,
                            "Show line numbers gutter in code editor",
                        );
                        ui.checkbox(
                            &mut self.settings.syntax_highlighting,
                            "Enable real-time Typst syntax highlighting",
                        );
                    }
                    SettingsTab::EditorAndDisk => {
                        ui.heading("Filesystem & Project Behavior");
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!(
                                "Active Project Directory: {}",
                                self.project.root_dir.display()
                            ))
                            .monospace()
                            .small(),
                        );
                        ui.add_space(6.0);
                        ui.checkbox(
                            &mut self.settings.auto_save_on_edit,
                            "Automatically save modified buffers to disk on edit",
                        );
                        if ui
                            .checkbox(
                                &mut self.settings.show_hidden_files,
                                "Show hidden dotfiles in project tree",
                            )
                            .changed()
                        {
                            self.project.show_hidden_files = self.settings.show_hidden_files;
                            rescan_needed = true;
                        }
                        ui.add_space(8.0);
                        if ui.button("Open Another Project Folder...").clicked() {
                            self.open_folder_dialog(ctx);
                        }
                    }
                    SettingsTab::CompilerAndPreview => {
                        ui.heading("Typst Compiler & Live Preview");
                        ui.add_space(8.0);
                        ui.checkbox(
                            &mut self.settings.auto_compile,
                            "Automatically compile Typst document while typing",
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Compile Debounce Delay:");
                            ui.add(
                                Slider::new(&mut self.settings.debounce_ms, 50..=800)
                                    .suffix(" ms"),
                            );
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Preview Rasterization DPI:");
                            if ui
                                .selectable_value(
                                    &mut self.settings.render_dpi_scale,
                                    1.5,
                                    "1.5x Fast",
                                )
                                .changed()
                            {
                                dpi_changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.settings.render_dpi_scale,
                                    2.0,
                                    "2.0x Crisp (Default)",
                                )
                                .changed()
                            {
                                dpi_changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.settings.render_dpi_scale,
                                    3.0,
                                    "3.0x Ultra HiDPI",
                                )
                                .changed()
                            {
                                dpi_changed = true;
                            }
                        });
                    }
                }

                ui.add_space(14.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        let prev_dark = self.settings.dark_mode;
                        self.settings = IdeSettings::default();
                        if prev_dark != self.settings.dark_mode {
                            theme_changed = true;
                        }
                        dpi_changed = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });

        self.show_settings_window = open && !close_clicked;

        if theme_changed {
            Self::apply_theme(ctx, self.settings.dark_mode);
        }
        if rescan_needed {
            let _ = self.project.rescan_tree();
        }
        if dpi_changed {
            self.trigger_compile_now(ctx);
        }
    }

    fn render_bottom_status_bar(&mut self, ctx: &Context) {
        let bar_bg = if self.settings.dark_mode {
            Color32::from_rgb(11, 15, 25)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
        let warn_count = self.diagnostics.iter().filter(|d| !d.is_error).count();

        TopBottomPanel::bottom("bottom_status_bar")
            .frame(
                Frame::new()
                    .fill(bar_bg)
                    .inner_margin(Margin::symmetric(12, 5))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (dot_color, status_text) = if self.compile_in_progress || self.pending_compile
                    {
                        (Color32::from_rgb(251, 191, 36), "Compiling...".to_string())
                    } else if error_count > 0 {
                        (
                            Color32::from_rgb(248, 113, 113),
                            format!("{} Error(s)", error_count),
                        )
                    } else {
                        (
                            Color32::from_rgb(52, 211, 153),
                            format!("Compiled ({:.1} ms)", self.compile_duration_ms),
                        )
                    };

                    ui.label(RichText::new(status_text).color(dot_color).strong().small());

                    if warn_count > 0 {
                        ui.separator();
                        ui.label(
                            RichText::new(format!("{} Warning(s)", warn_count))
                                .color(Color32::from_rgb(251, 191, 36))
                                .small(),
                        );
                    }

                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "Main: {}",
                            self.project.main_file.display()
                        ))
                        .small(),
                    );

                    ui.separator();
                    ui.label(
                        RichText::new(format!("Pages: {}", self.page_textures.len())).small(),
                    );

                    ui.separator();
                    if self.collab_ui.is_active {
                        let role = if self.collab_ui.is_host {
                            "Host"
                        } else {
                            "Peer"
                        };
                        let n_peers = self.collab_ui.connected_peer_count();
                        ui.label(
                            RichText::new(format!("P2P {}: {} peer(s)", role, n_peers))
                                .small()
                                .strong()
                                .color(Color32::from_rgb(56, 189, 248)),
                        );
                        ui.separator();
                    }
                    ui.label(RichText::new(&self.status_notice).small().weak());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(active) = self.project.active_buffer() {
                            let saved_state = if active.dirty { "Modified" } else { "Saved" };
                            ui.label(
                                RichText::new(format!(
                                    "{} ({})  |  {} lines  |  {} bytes",
                                    active.rel_path.display(),
                                    saved_state,
                                    active.line_count(),
                                    active.size_bytes
                                ))
                                .small()
                                .monospace(),
                            );
                        }
                    });
                });
            });
    }

    fn render_left_file_panel(&mut self, ctx: &Context) {
        if !self.show_sidebar {
            return;
        }

        let panel_bg = if self.settings.dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        SidePanel::left("left_file_tree_panel")
            .resizable(true)
            .default_width(250.0)
            .width_range(185.0..=400.0)
            .frame(
                Frame::new()
                    .fill(panel_bg)
                    .inner_margin(Margin::symmetric(10, 10))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("DISK PROJECT TREE")
                            .size(11.5)
                            .strong()
                            .color(if self.settings.dark_mode {
                                Color32::from_rgb(148, 163, 184)
                            } else {
                                Color32::from_rgb(71, 85, 105)
                            }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button("Refresh")
                            .on_hover_text("Rescan project folder from disk (F5)")
                            .clicked()
                        {
                            let _ = self.project.rescan_tree();
                        }
                        if ui
                            .small_button("+ File")
                            .on_hover_text("Create a new file on disk")
                            .clicked()
                        {
                            self.show_new_file_input = true;
                        }
                    });
                });

                ui.add_space(6.0);

                let mut clicked_path: Option<PathBuf> = None;
                let mut new_main_path: Option<PathBuf> = None;
                let mut rename_req: Option<PathBuf> = None;
                let mut delete_req: Option<PathBuf> = None;

                let active_path = self.project.active_file.clone();
                let main_path = self.project.main_file.clone();
                let project_title = format!("Folder: {}", self.project.project_name);
                let tree_snapshot = self.project.tree.clone();
                let presences_snapshot = self.collab_ui.remote_presences.clone();

                ScrollArea::vertical()
                    .id_salt("file_tree_scroll")
                    .show(ui, |ui| {
                        CollapsingHeader::new(RichText::new(project_title).strong())
                            .default_open(true)
                            .show(ui, |ui| {
                                for node in &tree_snapshot {
                                    Self::render_fs_tree_node(
                                        ui,
                                        node,
                                        &self.project,
                                        &active_path,
                                        &main_path,
                                        &presences_snapshot,
                                        &mut clicked_path,
                                        &mut new_main_path,
                                        &mut rename_req,
                                        &mut delete_req,
                                    );
                                }
                            });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);

                        if let Some(active) = self.project.active_buffer() {
                            ui.label(
                                RichText::new("DISK FILE DETAILS")
                                    .size(10.5)
                                    .strong()
                                    .color(Color32::from_rgb(100, 116, 139)),
                            );
                            ui.add_space(4.0);
                            Frame::new()
                                .fill(if self.settings.dark_mode {
                                    Color32::from_rgb(24, 33, 50)
                                } else {
                                    Color32::from_rgb(255, 255, 255)
                                })
                                .corner_radius(CornerRadius::same(5))
                                .inner_margin(Margin::symmetric(8, 8))
                                .stroke(Stroke::new(1.0_f32, border_col))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            active.kind.icon(),
                                            active.rel_path.display()
                                        ))
                                        .strong()
                                        .size(12.0),
                                    );
                                    ui.label(RichText::new(active.kind.label()).small().weak());
                                    ui.label(
                                        RichText::new(format!("Size: {} bytes", active.size_bytes))
                                            .small(),
                                    );
                                    if active.kind == FileKind::Typst {
                                        let is_main = active.rel_path == self.project.main_file;
                                        if is_main {
                                            ui.label(
                                                RichText::new("★ Main Compiler Entrypoint")
                                                    .small()
                                                    .color(Color32::from_rgb(56, 189, 248)),
                                            );
                                        } else if ui
                                            .small_button("Set as Main Entrypoint")
                                            .clicked()
                                        {
                                            new_main_path = Some(active.rel_path.clone());
                                        }
                                    }
                                });
                        }
                    });

                if let Some(rel_path) = clicked_path {
                    if let Err(e) = self.project.open_file(&rel_path) {
                        self.status_notice = format!("Could not open file: {}", e);
                    } else {
                        if let Some(buf) = self.project.active_buffer() {
                            self.inspect_asset_mode =
                                matches!(buf.kind, FileKind::Image | FileKind::Binary);
                        }
                        if self.settings.auto_compile {
                            self.trigger_compile_now(ctx);
                        }
                    }
                }
                if let Some(main_p) = new_main_path {
                    self.project.set_main_file(main_p.clone());
                    self.status_notice =
                        format!("Set '{}' as main Typst file", main_p.display());
                    self.trigger_compile_now(ctx);
                }
                if let Some(rel_to_rename) = rename_req {
                    self.rename_file_input = rel_to_rename.to_string_lossy().to_string();
                    self.rename_target_file = Some(rel_to_rename);
                }
                if let Some(rel_to_delete) = delete_req {
                    match self.project.delete_file_on_disk(&rel_to_delete) {
                        Ok(()) => {
                            if self.collab_ui.is_active {
                                self.collab_session.send(
                                    UiToSessionCmd::LocalFileDeleted {
                                        rel_path: rel_to_delete.clone(),
                                    },
                                );
                            }
                            self.status_notice =
                                format!("Deleted '{}' from disk", rel_to_delete.display());
                            self.trigger_compile_now(ctx);
                        }
                        Err(e) => {
                            self.status_notice = format!("Delete failed: {}", e);
                        }
                    }
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_fs_tree_node(
        ui: &mut Ui,
        node: &FsTreeNode,
        project: &DiskProject,
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
                CollapsingHeader::new(RichText::new(header).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for child in children {
                            Self::render_fs_tree_node(
                                ui,
                                child,
                                project,
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
                    let label_text =
                        format!("{} {}{}{}", kind.icon(), name, main_badge, dirty_mark);
                    let mut rich = RichText::new(label_text).size(12.5);
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
                            .on_hover_text(format!("{} is editing {}", peer.name, norm_rel));
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
                            && ui.button("Set as Main File").clicked()
                        {
                            *new_main_path = Some(rel_path.clone());
                            ui.close_menu();
                        }
                        if ui.button("Rename File...").clicked() {
                            *rename_req = Some(rel_path.clone());
                            ui.close_menu();
                        }
                        if !is_main && ui.button("Delete File").clicked() {
                            *delete_req = Some(rel_path.clone());
                            ui.close_menu();
                        }
                    });
                });
            }
        }
    }

    fn render_editor_pane(&mut self, ctx: &Context, ui: &mut Ui) {
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        // 1. Open File Tabs Bar
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let tabs = self.project.open_tabs.clone();
            let active_path = self.project.active_file.clone();
            let mut select_tab = None;
            let mut close_tab = None;

            for tab_path in &tabs {
                let is_active = *tab_path == active_path;
                let short_name = tab_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");
                let icon = FileKind::from_path(tab_path).icon();
                let dirty = self
                    .project
                    .buffers
                    .get(tab_path)
                    .map(|b| b.dirty)
                    .unwrap_or(false);
                let dirty_suffix = if dirty { " *" } else { "" };
                let tab_label = format!("{} {}{}", icon, short_name, dirty_suffix);

                if ui
                    .selectable_label(is_active, RichText::new(tab_label).size(12.0))
                    .on_hover_text(self.project.root_dir.join(tab_path).display().to_string())
                    .clicked()
                {
                    select_tab = Some(tab_path.clone());
                }
                if tabs.len() > 1
                    && ui
                        .small_button("×")
                        .on_hover_text(format!("Close tab {}", short_name))
                        .clicked()
                {
                    close_tab = Some(tab_path.clone());
                }
                ui.separator();
            }

            if let Some(p) = select_tab {
                let _ = self.project.open_file(&p);
                if self.settings.auto_compile {
                    self.trigger_compile_now(ctx);
                }
            }
            if let Some(p) = close_tab {
                self.project.close_tab(&p);
                if self.settings.auto_compile {
                    self.trigger_compile_now(ctx);
                }
            }
        });

        ui.separator();

        // 2. Quick Formatting & Snippet Toolbar
        let active_path = self.project.active_file.clone();
        let active_kind = FileKind::from_path(&active_path);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let mut snippet_to_insert: Option<&'static str> = None;

            if active_kind == FileKind::Typst {
                if ui
                    .small_button("= Heading")
                    .on_hover_text("Append a section heading")
                    .clicked()
                {
                    snippet_to_insert = Some("\n\n== New Section\n");
                }
                if ui
                    .small_button("*Bold*")
                    .on_hover_text("Insert bold markup")
                    .clicked()
                {
                    snippet_to_insert = Some(" *bold text* ");
                }
                if ui
                    .small_button("_Italic_")
                    .on_hover_text("Insert italic markup")
                    .clicked()
                {
                    snippet_to_insert = Some(" _emphasized_ ");
                }
                if ui
                    .small_button("$ Math $")
                    .on_hover_text("Insert a display math equation block")
                    .clicked()
                {
                    snippet_to_insert =
                        Some("\n$ f(x) = integral_0^x e^(-t^2) dif t $\n");
                }
                if ui
                    .small_button("+ Table")
                    .on_hover_text("Insert a formatted table snippet")
                    .clicked()
                {
                    snippet_to_insert = Some(
                        "\n#table(\n  columns: (1fr, 1fr),\n  [*Metric*], [*Value*],\n  [Throughput], [60 FPS],\n)\n",
                    );
                }
                if ui
                    .small_button("+ Figure")
                    .on_hover_text("Insert figure with assets/diagram.svg")
                    .clicked()
                {
                    snippet_to_insert = Some(
                        "\n#figure(\n  image(\"assets/diagram.svg\", width: 85%),\n  caption: [Architecture diagram.],\n)\n",
                    );
                }
            } else if active_kind == FileKind::Svg {
                let mode_text = if self.inspect_asset_mode {
                    "Edit SVG XML"
                } else {
                    "Asset Metadata"
                };
                if ui.small_button(mode_text).clicked() {
                    self.inspect_asset_mode = !self.inspect_asset_mode;
                }
            }

            if let Some(snippet) = snippet_to_insert {
                let auto_save = self.settings.auto_save_on_edit;
                let mut updated_text = None;
                if let Some(file) = self.project.active_buffer_mut() {
                    if let Some(ref mut text) = file.text {
                        text.push_str(snippet);
                        updated_text = Some(text.clone());
                        self.project.mark_active_modified(auto_save);
                        self.pending_compile = true;
                        self.last_edit_time = ctx.input(|i| i.time);
                    }
                }
                if let (true, Some(new_text)) = (self.collab_ui.is_active, updated_text) {
                    self.collab_session.send(UiToSessionCmd::LocalTextEdited {
                        rel_path: active_path.clone(),
                        new_text,
                    });
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("A+")
                    .on_hover_text("Increase editor font size")
                    .clicked()
                {
                    self.settings.editor_font_size =
                        (self.settings.editor_font_size + 1.0).clamp(10.0, 22.0);
                }
                if ui
                    .small_button("A-")
                    .on_hover_text("Decrease editor font size")
                    .clicked()
                {
                    self.settings.editor_font_size =
                        (self.settings.editor_font_size - 1.0).clamp(10.0, 22.0);
                }
                ui.label(
                    RichText::new(format!("FILE: {}", active_path.display()))
                        .size(11.0)
                        .monospace()
                        .weak(),
                );
            });
        });

        ui.separator();

        // 3. Editor Area or Asset Inspector
        let dark_mode = self.settings.dark_mode;
        let font_size = self.settings.editor_font_size;
        let show_line_numbers = self.settings.show_line_numbers;
        let use_syntax_highlight = self.settings.syntax_highlighting;
        let inspect_asset_mode = self.inspect_asset_mode;
        let peers_in_active_file: Vec<PeerPresence> = self
            .collab_ui
            .peers_in_file(&active_path)
            .into_iter()
            .cloned()
            .collect();
        let restore_cursor = self.pending_cursor_restore.take();

        let Some(file) = self.project.active_buffer_mut() else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a file from the Disk Project Tree.");
            });
            return;
        };

        if file.text.is_none() || inspect_asset_mode {
            let mut exit_inspect = false;
            Frame::new()
                .fill(if dark_mode {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::from_rgb(248, 250, 252)
                })
                .inner_margin(Margin::same(16))
                .corner_radius(CornerRadius::same(6))
                .stroke(Stroke::new(1.0_f32, border_col))
                .show(ui, |ui| {
                    ui.heading(format!("{} {}", file.kind.icon(), file.rel_path.display()));
                    ui.add_space(6.0);
                    ui.label(format!("Disk Path: {}", file.abs_path.display()));
                    ui.label(format!("Asset Type: {}", file.kind.label()));
                    ui.label(format!("File Size: {} bytes", file.size_bytes));
                    ui.add_space(8.0);
                    ui.label("Embed in any Typst file using:");
                    ui.code(format!(
                        "#image(\"{}\", width: 80%)",
                        file.rel_path.to_string_lossy().replace('\\', "/")
                    ));
                    if file.text.is_some() {
                        ui.add_space(8.0);
                        if ui.button("Switch to Source Code Editor").clicked() {
                            exit_inspect = true;
                        }
                    }
                });
            if exit_inspect {
                self.inspect_asset_mode = false;
            }
            return;
        }

        let is_typst = file.kind == FileKind::Typst && use_syntax_highlight;
        let mut text_changed = false;
        let mut edited_text_snapshot: Option<String> = None;
        let mut observed_cursor: Option<CharSelection> = None;

        let editor_bg = if dark_mode {
            Color32::from_rgb(11, 15, 25)
        } else {
            Color32::from_rgb(255, 255, 255)
        };
        let gutter_bg = if dark_mode {
            Color32::from_rgb(17, 24, 39)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let gutter_fg = if dark_mode {
            Color32::from_rgb(100, 116, 139)
        } else {
            Color32::from_rgb(148, 163, 184)
        };

        Frame::new()
            .fill(editor_bg)
            .stroke(Stroke::new(1.0_f32, border_col))
            .corner_radius(CornerRadius::same(4))
            .show(ui, |ui| {
                ScrollArea::both()
                    .id_salt("code_editor_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let text_ref = file.text.as_mut().unwrap();
                        let line_count = text_ref.lines().count().max(1);
                        let line_numbers_str: String =
                            (1..=line_count).map(|i| format!("{:>3}\n", i)).collect();

                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;

                            if show_line_numbers {
                                Frame::new()
                                    .fill(gutter_bg)
                                    .inner_margin(Margin::symmetric(8, 8))
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(line_numbers_str.trim_end())
                                                    .font(FontId::new(
                                                        font_size,
                                                        FontFamily::Monospace,
                                                    ))
                                                    .color(gutter_fg),
                                            )
                                            .selectable(false),
                                        );
                                    });
                            }

                            let mut layouter = |ui: &Ui, string: &str, wrap_width: f32| {
                                let mut job = if is_typst {
                                    highlight_typst_code(string, dark_mode, font_size)
                                } else {
                                    LayoutJob::simple(
                                        string.to_owned(),
                                        FontId::new(font_size, FontFamily::Monospace),
                                        if dark_mode {
                                            Color32::from_rgb(226, 232, 240)
                                        } else {
                                            Color32::from_rgb(30, 41, 59)
                                        },
                                        wrap_width,
                                    )
                                };
                                job.wrap.max_width = f32::INFINITY;
                                ui.fonts(|f| f.layout_job(job))
                            };

                            Frame::new()
                                .fill(editor_bg)
                                .inner_margin(Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    let mut output = TextEdit::multiline(text_ref)
                                        .id_salt("typst_code_editor")
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(34)
                                        .frame(false)
                                        .font(FontId::new(font_size, FontFamily::Monospace))
                                        .layouter(&mut layouter)
                                        .show(ui);

                                    // Restore StickyIndex-preserved cursor after a remote CRDT update
                                    if let Some(sel) = restore_cursor {
                                        output.state.cursor.set_char_range(Some(
                                            CCursorRange::two(
                                                CCursor::new(sel.secondary_char),
                                                CCursor::new(sel.primary_char),
                                            ),
                                        ));
                                        output.state.clone().store(ctx, output.response.id);
                                    }

                                    if let Some(range) = output.cursor_range {
                                        observed_cursor = Some(CharSelection {
                                            primary_char: range.primary.ccursor.index,
                                            secondary_char: range.secondary.ccursor.index,
                                        });
                                    }

                                    // Render remote collaborators' colored cursors, selections, and name tags
                                    if !peers_in_active_file.is_empty() {
                                        let peer_refs: Vec<&PeerPresence> =
                                            peers_in_active_file.iter().collect();
                                        paint_remote_cursors_in_editor(ui, &output, &peer_refs);
                                    }

                                    let ime_in_progress = ctx.input(|i| {
                                        i.events.iter().any(|ev| {
                                            matches!(
                                                ev,
                                                egui::Event::Ime(egui::ImeEvent::Preedit(_))
                                            )
                                        })
                                    });

                                    if output.response.changed() && !ime_in_progress {
                                        text_changed = true;
                                        edited_text_snapshot = Some(text_ref.clone());
                                    }
                                });
                        });
                    });
            });

        if text_changed {
            let auto_save = self.settings.auto_save_on_edit;
            self.project.mark_active_modified(auto_save);
            if let (true, Some(new_text)) = (self.collab_ui.is_active, edited_text_snapshot) {
                self.collab_session.send(UiToSessionCmd::LocalTextEdited {
                    rel_path: active_path.clone(),
                    new_text,
                });
            }
            if self.settings.auto_compile {
                self.pending_compile = true;
                self.last_edit_time = ctx.input(|i| i.time);
            }
        }

        if self.collab_ui.is_active {
            if let Some(sel) = observed_cursor {
                let candidate = (active_path.clone(), sel);
                if self.last_sent_cursor.as_ref() != Some(&candidate) {
                    self.last_sent_cursor = Some(candidate);
                    self.collab_session
                        .send(UiToSessionCmd::LocalCursorUpdated {
                            active_rel_path: Some(active_path),
                            selection: Some(sel),
                        });
                }
            }
        }
    }

    fn render_preview_pane(&mut self, _ctx: &Context, ui: &mut Ui) {
        let canvas_bg = if self.settings.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let border_col = if self.settings.dark_mode {
            Color32::from_rgb(51, 65, 85)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("LIVE DOCUMENT PREVIEW")
                    .size(12.0)
                    .strong()
                    .color(if self.settings.dark_mode {
                        Color32::from_rgb(148, 163, 184)
                    } else {
                        Color32::from_rgb(71, 85, 105)
                    }),
            );

            ui.separator();
            ui.label(
                RichText::new(format!("{} Page(s)", self.page_textures.len()))
                    .size(12.0)
                    .strong(),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("Fit Width")
                    .on_hover_text("Reset preview zoom to 100%")
                    .clicked()
                {
                    self.zoom = 1.0;
                }
                ui.label(
                    RichText::new(format!("Scale: {:.0}%", self.zoom * 100.0))
                        .size(11.5)
                        .monospace(),
                );
            });
        });

        ui.separator();

        let has_diagnostics = !self.diagnostics.is_empty();
        let available_height = ui.available_height();
        let banner_height = if has_diagnostics {
            (available_height * 0.28).clamp(110.0, 195.0)
        } else {
            0.0
        };
        let preview_height = (available_height - banner_height - 4.0).max(120.0);

        ui.allocate_ui(Vec2::new(ui.available_width(), preview_height), |ui| {
            Frame::new()
                .fill(canvas_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(4))
                .show(ui, |ui| {
                    if ui.ui_contains_pointer() {
                        let zoom_delta = ui.input(|i| i.zoom_delta());
                        if (zoom_delta - 1.0).abs() > 0.001 {
                            self.zoom = (self.zoom * zoom_delta).clamp(0.4, 2.5);
                        }
                    }

                    ScrollArea::both()
                        .id_salt("typst_preview_scroll_area")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(18.0);

                                if self.page_textures.is_empty() {
                                    ui.add_space(40.0);
                                    ui.label(
                                        RichText::new("No rendered pages available.")
                                            .size(14.0)
                                            .weak(),
                                    );
                                } else {
                                    let avail_w = (ui.available_width() - 36.0).max(200.0);

                                    for page in &self.page_textures {
                                        let base_scale =
                                            (avail_w / page.width_pt).clamp(0.45, 1.35);
                                        let display_w = page.width_pt * base_scale * self.zoom;
                                        let display_h = page.height_pt * base_scale * self.zoom;
                                        let page_size = Vec2::new(display_w, display_h);

                                        let (rect, _response) =
                                            ui.allocate_exact_size(page_size, Sense::hover());

                                        let painter = ui.painter();
                                        let shadow_rect_outer =
                                            rect.translate(Vec2::new(0.0, 5.0)).expand(3.0);
                                        painter.rect_filled(
                                            shadow_rect_outer,
                                            CornerRadius::same(3),
                                            Color32::from_black_alpha(55),
                                        );
                                        let shadow_rect_inner =
                                            rect.translate(Vec2::new(0.0, 2.0)).expand(1.0);
                                        painter.rect_filled(
                                            shadow_rect_inner,
                                            CornerRadius::same(2),
                                            Color32::from_black_alpha(85),
                                        );

                                        painter.rect_filled(
                                            rect,
                                            CornerRadius::ZERO,
                                            Color32::WHITE,
                                        );
                                        painter.image(
                                            page.texture.id(),
                                            rect,
                                            Rect::from_min_max(
                                                Pos2::new(0.0, 0.0),
                                                Pos2::new(1.0, 1.0),
                                            ),
                                            Color32::WHITE,
                                        );
                                        painter.rect_stroke(
                                            rect,
                                            CornerRadius::ZERO,
                                            Stroke::new(1.0_f32, Color32::from_rgb(148, 163, 184)),
                                            StrokeKind::Outside,
                                        );

                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "Page {} of {}",
                                                page.page_number,
                                                self.page_textures.len()
                                            ))
                                            .size(11.0)
                                            .weak(),
                                        );
                                        ui.add_space(18.0);
                                    }
                                }
                            });
                        });
                });
        });

        if has_diagnostics {
            ui.add_space(4.0);
            let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
            let banner_bg = if error_count > 0 {
                if self.settings.dark_mode {
                    Color32::from_rgb(45, 18, 24)
                } else {
                    Color32::from_rgb(254, 242, 242)
                }
            } else if self.settings.dark_mode {
                Color32::from_rgb(45, 34, 15)
            } else {
                Color32::from_rgb(255, 251, 235)
            };

            let banner_border = if error_count > 0 {
                Color32::from_rgb(239, 68, 68)
            } else {
                Color32::from_rgb(245, 158, 11)
            };

            let mut file_to_open: Option<PathBuf> = None;

            Frame::new()
                .fill(banner_bg)
                .stroke(Stroke::new(1.5_f32, banner_border))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let title = if error_count > 0 {
                            format!("[!] Typst Compiler Diagnostics ({} Error(s))", error_count)
                        } else {
                            format!("[!] Typst Compiler Warnings ({})", self.diagnostics.len())
                        };
                        ui.label(
                            RichText::new(title)
                                .strong()
                                .color(banner_border)
                                .size(12.5),
                        );
                    });
                    ui.separator();

                    ScrollArea::vertical()
                        .id_salt("diagnostics_banner_scroll")
                        .max_height(banner_height - 38.0)
                        .show(ui, |ui| {
                            for diag in &self.diagnostics {
                                ui.horizontal_wrapped(|ui| {
                                    let badge_color = if diag.is_error {
                                        Color32::from_rgb(248, 113, 113)
                                    } else {
                                        Color32::from_rgb(251, 191, 36)
                                    };
                                    let loc = match (diag.line, diag.column) {
                                        (Some(l), Some(c)) => {
                                            format!("{}:{}:{}", diag.file.display(), l, c)
                                        }
                                        (Some(l), None) => format!("{}:{}", diag.file.display(), l),
                                        _ => diag.file.display().to_string(),
                                    };
                                    if ui
                                        .small_button(
                                            RichText::new(&loc).monospace().color(badge_color),
                                        )
                                        .on_hover_text("Click to open file in editor")
                                        .clicked()
                                    {
                                        file_to_open = Some(diag.file.clone());
                                    }
                                    ui.label(RichText::new(&diag.message).strong().size(12.0));
                                });
                                for hint in &diag.hints {
                                    ui.label(
                                        RichText::new(format!("  Hint: {}", hint))
                                            .italics()
                                            .size(11.5),
                                    );
                                }
                                ui.add_space(3.0);
                            }
                        });
                });

            if let Some(target_file) = file_to_open {
                let _ = self.project.open_file(&target_file);
            }
        }
    }

    fn handle_screenshot_lifecycle(&mut self, ctx: &Context) {
        let Some(ref target_path) = self.screenshot_path else {
            return;
        };

        ctx.request_repaint();
        self.frame_count = self.frame_count.saturating_add(1);

        let mut captured_image: Option<Arc<egui::ColorImage>> = None;
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    captured_image = Some(Arc::clone(image));
                }
            }
        });

        if let Some(img) = captured_image {
            if let Some(parent) = target_path.parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            let width = img.width() as u32;
            let height = img.height() as u32;
            let raw_rgba = img.as_raw();
            if let Err(err) = image::save_buffer(
                target_path,
                raw_rgba,
                width,
                height,
                image::ColorType::Rgba8,
            ) {
                eprintln!("Failed to save screenshot to {:?}: {}", target_path, err);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if self.frame_count >= 120 && !self.screenshot_requested {
            self.screenshot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }
}

impl eframe::App for TypstIdeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_channels(ctx);
        self.handle_keyboard_shortcuts(ctx);

        // Debounced non-blocking compilation
        if self.pending_compile && self.settings.auto_compile {
            let now = ctx.input(|i| i.time);
            let delay_secs = (self.settings.debounce_ms as f64) / 1000.0;
            if now - self.last_edit_time >= delay_secs {
                self.trigger_compile_now(ctx);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }

        // Desktop Menu Bar on Linux & Windows (on macOS, menu bar is rendered by the OS at the top of the screen)
        #[cfg(not(target_os = "macos"))]
        self.render_desktop_menu_bar(ctx);

        // Top Action Toolbar
        self.render_top_toolbar(ctx);

        // Bottom Status Bar
        self.render_bottom_status_bar(ctx);

        // Collapsible Left File Overview Panel (Disk Project Tree)
        self.render_left_file_panel(ctx);

        // Main Split View (Code Editor Left + Live Preview Right)
        let central_bg = if self.settings.dark_mode {
            Color32::from_rgb(17, 24, 39)
        } else {
            Color32::from_rgb(248, 250, 252)
        };

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(central_bg)
                    .inner_margin(Margin::symmetric(10, 8)),
            )
            .show(ctx, |ui| {
                let total_rect = ui.available_rect_before_wrap();
                let spacing = 10.0;
                let half_w = ((total_rect.width() - spacing) * 0.5).max(160.0);
                let full_h = total_rect.height();

                let left_rect = Rect::from_min_size(total_rect.min, Vec2::new(half_w, full_h));
                let div_x = total_rect.min.x + half_w + spacing * 0.5;
                let right_rect = Rect::from_min_size(
                    Pos2::new(total_rect.min.x + half_w + spacing, total_rect.min.y),
                    Vec2::new((total_rect.width() - half_w - spacing).max(160.0), full_h),
                );

                // Left Pane: Code Editor
                let mut left_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(left_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                self.render_editor_pane(ctx, &mut left_ui);

                // Vertical Split Divider Line
                let div_color = if self.settings.dark_mode {
                    Color32::from_rgb(45, 55, 72)
                } else {
                    Color32::from_rgb(203, 213, 225)
                };
                ui.painter().line_segment(
                    [
                        Pos2::new(div_x, total_rect.min.y),
                        Pos2::new(div_x, total_rect.max.y),
                    ],
                    Stroke::new(1.0_f32, div_color),
                );

                // Right Pane: Live Document Preview
                let mut right_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(right_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                self.render_preview_pane(ctx, &mut right_ui);

                ui.advance_cursor_after_rect(total_rect);
            });

        // Settings Sub-Window (opened via File > Settings)
        self.render_settings_subwindow(ctx);

        // Export PDF Dialog (opened via Export PDF... button or File > Export PDF...)
        self.render_export_pdf_dialog(ctx);

        // P2P Collaboration Sub-Window (opened via Project > Collaboration (P2P)...)
        render_collaboration_window(
            ctx,
            self.settings.dark_mode,
            &self.project.root_dir,
            &mut self.settings.user_name,
            &mut self.settings.user_color,
            &mut self.collab_ui,
            &self.collab_session,
        );

        self.handle_screenshot_lifecycle(ctx);
    }
}

/// Native macOS screen-top menu bar using `muda`, so on macOS the menu bar is part of the OS
/// directly at the top of the screen rather than rendered inside the window.
#[cfg(target_os = "macos")]
mod macos_native_menu {
    use super::MenuAction;
    use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    pub struct MacOsMenuBar {
        _menu: Menu,
        id_new_file: MenuId,
        id_open_folder: MenuId,
        id_rescan: MenuId,
        id_save: MenuId,
        id_save_all: MenuId,
        id_export_pdf: MenuId,
        id_settings: MenuId,
        id_collab: MenuId,
        id_quit: MenuId,
        id_toggle_sidebar: MenuId,
        id_toggle_theme: MenuId,
        id_zoom_in: MenuId,
        id_zoom_out: MenuId,
        id_zoom_reset: MenuId,
        id_compile: MenuId,
    }

    impl MacOsMenuBar {
        pub fn init() -> Option<Self> {
            let menu = Menu::new();

            let file_menu = Submenu::new("File", true);
            let item_new_file = MenuItem::new("New File in Project...", true, None);
            let item_open_folder = MenuItem::new("Open Project Folder...", true, None);
            let item_rescan = MenuItem::new("Rescan Folder on Disk", true, None);
            let item_save = MenuItem::new("Save Active File", true, None);
            let item_save_all = MenuItem::new("Save All Project Files", true, None);
            let item_export_pdf = MenuItem::new("Export PDF...", true, None);
            let item_settings = MenuItem::new("Settings...", true, None);
            let item_quit = MenuItem::new("Quit Typst Studio", true, None);

            let _ = file_menu.append_items(&[
                &item_new_file,
                &item_open_folder,
                &item_rescan,
                &PredefinedMenuItem::separator(),
                &item_save,
                &item_save_all,
                &item_export_pdf,
                &PredefinedMenuItem::separator(),
                &item_settings,
                &PredefinedMenuItem::separator(),
                &item_quit,
            ]);

            let view_menu = Submenu::new("View", true);
            let item_toggle_sidebar = MenuItem::new("Toggle Project File Tree", true, None);
            let item_toggle_theme = MenuItem::new("Switch Theme (Dark / Light)", true, None);
            let item_zoom_in = MenuItem::new("Zoom In (+)", true, None);
            let item_zoom_out = MenuItem::new("Zoom Out (-)", true, None);
            let item_zoom_reset = MenuItem::new("Reset Zoom (100%)", true, None);
            let _ = view_menu.append_items(&[
                &item_toggle_sidebar,
                &item_toggle_theme,
                &PredefinedMenuItem::separator(),
                &item_zoom_in,
                &item_zoom_out,
                &item_zoom_reset,
            ]);

            let project_menu = Submenu::new("Project", true);
            let item_collab = MenuItem::new("Collaboration (P2P)...", true, None);
            let item_compile = MenuItem::new("Compile Document Now", true, None);
            let _ = project_menu.append_items(&[
                &item_collab,
                &PredefinedMenuItem::separator(),
                &item_compile,
            ]);

            let _ = menu.append_items(&[&file_menu, &view_menu, &project_menu]);
            menu.init_for_nsapp();

            Some(Self {
                _menu: menu,
                id_new_file: item_new_file.id().clone(),
                id_open_folder: item_open_folder.id().clone(),
                id_rescan: item_rescan.id().clone(),
                id_save: item_save.id().clone(),
                id_save_all: item_save_all.id().clone(),
                id_export_pdf: item_export_pdf.id().clone(),
                id_settings: item_settings.id().clone(),
                id_collab: item_collab.id().clone(),
                id_quit: item_quit.id().clone(),
                id_toggle_sidebar: item_toggle_sidebar.id().clone(),
                id_toggle_theme: item_toggle_theme.id().clone(),
                id_zoom_in: item_zoom_in.id().clone(),
                id_zoom_out: item_zoom_out.id().clone(),
                id_zoom_reset: item_zoom_reset.id().clone(),
                id_compile: item_compile.id().clone(),
            })
        }

        pub fn poll_action(&self) -> Option<MenuAction> {
            let ev = MenuEvent::receiver().try_recv().ok()?;
            let id = ev.id();
            if *id == self.id_new_file {
                Some(MenuAction::NewFile)
            } else if *id == self.id_open_folder {
                Some(MenuAction::OpenFolder)
            } else if *id == self.id_rescan {
                Some(MenuAction::RescanFolder)
            } else if *id == self.id_save {
                Some(MenuAction::SaveFile)
            } else if *id == self.id_save_all {
                Some(MenuAction::SaveAll)
            } else if *id == self.id_export_pdf {
                Some(MenuAction::ExportPdf)
            } else if *id == self.id_settings {
                Some(MenuAction::OpenSettings)
            } else if *id == self.id_collab {
                Some(MenuAction::OpenCollab)
            } else if *id == self.id_quit {
                Some(MenuAction::Quit)
            } else if *id == self.id_toggle_sidebar {
                Some(MenuAction::ToggleSidebar)
            } else if *id == self.id_toggle_theme {
                Some(MenuAction::ToggleTheme)
            } else if *id == self.id_zoom_in {
                Some(MenuAction::ZoomIn)
            } else if *id == self.id_zoom_out {
                Some(MenuAction::ZoomOut)
            } else if *id == self.id_zoom_reset {
                Some(MenuAction::ZoomReset)
            } else if *id == self.id_compile {
                Some(MenuAction::CompileNow)
            } else {
                None
            }
        }
    }
}

mod pollster {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    pub fn block_on_or_spin<F: Future>(mut fut: F) -> F::Output {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);

        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut cx = Context::from_waker(&waker);

        loop {
            let pinned = unsafe { Pin::new_unchecked(&mut fut) };
            match pinned.poll(&mut cx) {
                Poll::Ready(val) => return val,
                Poll::Pending => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    }
}
