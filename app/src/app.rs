use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{
    Align, Align2, CollapsingHeader, Color32, Context, CornerRadius, CursorIcon, FontFamily,
    FontId, Frame, Key, Layout, Margin, Modifiers, Pos2, Rect, RichText, ScrollArea, Sense,
    SidePanel, Stroke, StrokeKind, TextEdit, TopBottomPanel, Ui, Vec2,
};

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::{normalize_rel_path, CharSelection};
use crate::collab::session::{CollabSessionHandle, SessionToUiEvent, UiToSessionCmd};
use crate::collab::ui::{
    paint_remote_cursors_in_editor, render_collaboration_window, CollabUiState,
};
use crate::compiler::{
    compile_project, export_pdf_bytes_with_config, upload_page_textures, CompileSnapshot,
    CompiledOutlineItem, DiagnosticItem, PdfExportConfig, RenderedPageData, RenderedPageTexture,
};
use crate::highlight::{highlight_typst_code, ThemePalette};
use crate::project::{ensure_sample_project_on_disk, DiskProject, FileKind, FsTreeNode};
use crate::world::SharedWorldResources;
use typst::syntax::SyntaxNode;

#[cfg(target_os = "macos")]
use crate::platform::menu::MacOsMenuBar;
pub use crate::platform::menu::MenuAction;

pub use crate::core::outline::{
    assign_hierarchical_numbers, build_outline_tree, extract_document_outline,
    extract_typst_ast_outline, extract_typst_markup_plain_text, DocumentOutlineItem,
    OutlineTreeNode,
};
pub use crate::core::settings::{IdeSettings, SettingsTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorContextAction {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    WrapOrInsert(&'static str, &'static str, &'static str),
    InsertSnippet(&'static str),
    Menu(MenuAction),
    SetActiveAsMain,
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
    pub editor_split_ratio: f32,
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
    pub pending_scroll_to_char: Option<usize>,
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
    pub compiled_outline: Vec<CompiledOutlineItem>,
    pub diagnostics: Vec<DiagnosticItem>,

    // New file creation & rename modal/inline state
    pub show_new_file_input: bool,
    pub new_file_name: String,
    pub rename_target_file: Option<PathBuf>,
    pub rename_file_input: String,
    pub inspect_asset_mode: bool,

    // Editor right-click context menu & selection preservation state
    pub last_editor_selection: Option<CharSelection>,
    pub clipboard_fallback: String,
    pub pending_editor_context_action: Option<EditorContextAction>,

    // Status bar notification
    pub status_notice: String,

    // Channels for native file dialogs & background compilation
    pub io_tx: Sender<IoEvent>,
    pub io_rx: Receiver<IoEvent>,
    pub compile_tx: Sender<CompileSnapshot>,
    pub compile_rx: Receiver<CompileSnapshot>,

    // Native macOS OS Menu Bar handle
    #[cfg(target_os = "macos")]
    pub macos_menu: Option<MacOsMenuBar>,

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

        let initial_snapshot =
            compile_project(Arc::clone(&resources), &project, settings.render_dpi_scale);
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
        let macos_menu = MacOsMenuBar::init();

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
            compiled_outline: initial_snapshot.outline,
            diagnostics: initial_snapshot.diagnostics,
            project,
            settings,
            show_sidebar: true,
            editor_split_ratio: 0.5,
            zoom: 1.0,
            show_settings_window: false,
            settings_tab: SettingsTab::Appearance,
            show_export_pdf_dialog: false,
            pdf_export_config: PdfExportConfig::default(),
            export_pdf_error: None,
            collab_ui: CollabUiState::default(),
            collab_session,
            pending_cursor_restore: None,
            pending_scroll_to_char: None,
            last_sent_cursor: None,
            show_new_file_input: false,
            new_file_name: "chapters/appendix.typ".to_string(),
            rename_target_file: None,
            rename_file_input: String::new(),
            inspect_asset_mode: false,
            last_editor_selection: None,
            clipboard_fallback: String::new(),
            pending_editor_context_action: None,
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
        crate::ui::theme::apply_theme(ctx, dark_mode);
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
                    self.status_notice = format!(
                        "Rescanned disk folder '{}'",
                        self.project.root_dir.display()
                    );
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
            MenuAction::ToggleWordWrap => {
                self.settings.word_wrap = !self.settings.word_wrap;
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
        if snapshot.document.is_some() {
            self.compiled_outline = snapshot.outline;
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
                self.collab_session.send(UiToSessionCmd::LocalRedo {
                    rel_path: active_rel.clone(),
                });
            } else if collab_active && i.consume_key(Modifiers::COMMAND, Key::Y) {
                self.collab_session.send(UiToSessionCmd::LocalRedo {
                    rel_path: active_rel.clone(),
                });
            } else if collab_active && i.consume_key(Modifiers::COMMAND, Key::Z) {
                self.collab_session.send(UiToSessionCmd::LocalUndo {
                    rel_path: active_rel.clone(),
                });
            }
        });
        if let Some(action) = triggered {
            self.dispatch_menu_action(ctx, action);
        }
    }

    fn open_folder_dialog(&self, ctx: &Context) {
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();
        crate::platform::dialogs::spawn_open_folder_dialog(
            &self.project.root_dir,
            move |path| {
                let _ = tx.send(IoEvent::FolderPicked(path));
            },
            move || ctx_clone.request_repaint(),
        );
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

                crate::platform::dialogs::spawn_save_file_dialog(
                    &start_dir,
                    &default_pdf_name,
                    "PDF Document",
                    "pdf",
                    pdf_bytes,
                    move |res| match res {
                        Ok(out_path) => {
                            let _ = tx.send(IoEvent::StatusMessage(format!(
                                "Exported {} to '{}' ({:.1} KB)",
                                badge,
                                out_path.display(),
                                size_kb
                            )));
                        }
                        Err(err) => {
                            let _ = tx
                                .send(IoEvent::StatusMessage(format!("PDF write failed: {}", err)));
                        }
                    },
                    move || ctx_clone.request_repaint(),
                );
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

    /// Renders the Export PDF Dialog as a real OS sub-window allowing the user to select the PDF version
    /// (1.4, 1.5, 1.6, 1.7, 2.0) and PDF standard(s) (PDF/A-1b, PDF/A-1a, PDF/A-2b, PDF/A-2u,
    /// PDF/A-2a, PDF/A-3b, PDF/A-3u, PDF/A-3a, PDF/A-4, PDF/UA-1).
    fn render_export_pdf_dialog(&mut self, ctx: &Context) {
        let entry_file = self.project.effective_compile_entrypoint();
        let trigger_export = crate::ui::dialogs::export_pdf::render_export_pdf_dialog(
            ctx,
            &mut self.show_export_pdf_dialog,
            &mut self.pdf_export_config,
            &mut self.export_pdf_error,
            &entry_file,
            self.settings.dark_mode,
        );
        if trigger_export {
            self.export_pdf_action(ctx);
        }
    }

    /// Renders the desktop application menu bar on Linux and Windows.
    /// On macOS, the menu bar is part of the OS at the top of the screen via `muda` and is NOT rendered in-window.
    #[cfg(not(target_os = "macos"))]
    fn render_desktop_menu_bar(&mut self, ctx: &Context) {
        let palette = ThemePalette::for_mode(self.settings.dark_mode);
        let menu_bg = palette.chrome_sidebar;
        let border_col = palette.panel_border;

        let mut pending_action: Option<MenuAction> = None;
        let mut snippet_to_insert: Option<&'static str> = None;
        let mut editor_action: Option<EditorContextAction> = None;

        TopBottomPanel::top("desktop_os_menu_bar")
            .frame(
                Frame::new()
                    .fill(menu_bg)
                    .corner_radius(CornerRadius::ZERO)
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
                        ui.set_min_width(210.0);
                        if ui.button("Undo  (Ctrl+Z)").clicked() {
                            editor_action = Some(EditorContextAction::Undo);
                            ui.close_menu();
                        }
                        if ui.button("Redo  (Ctrl+Y)").clicked() {
                            editor_action = Some(EditorContextAction::Redo);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Cut  (Ctrl+X)").clicked() {
                            editor_action = Some(EditorContextAction::Cut);
                            ui.close_menu();
                        }
                        if ui.button("Copy  (Ctrl+C)").clicked() {
                            editor_action = Some(EditorContextAction::Copy);
                            ui.close_menu();
                        }
                        if ui.button("Paste  (Ctrl+V)").clicked() {
                            editor_action = Some(EditorContextAction::Paste);
                            ui.close_menu();
                        }
                        if ui.button("Select All  (Ctrl+A)").clicked() {
                            editor_action = Some(EditorContextAction::SelectAll);
                            ui.close_menu();
                        }
                        ui.separator();
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
                        let wrap_txt = if self.settings.word_wrap {
                            "Disable Automatic Line Wrapping"
                        } else {
                            "Enable Automatic Line Wrapping"
                        };
                        if ui.button(wrap_txt).clicked() {
                            pending_action = Some(MenuAction::ToggleWordWrap);
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

        if let Some(act) = editor_action {
            self.pending_editor_context_action = Some(act);
        }

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

    /// Renders the New File and Rename File dialogs as real OS sub-windows.
    fn render_file_dialogs_subwindows(&mut self, ctx: &Context) {
        if let Some(new_file_str) = crate::ui::dialogs::file_modals::render_new_file_modal(
            ctx,
            &mut self.show_new_file_input,
            &mut self.new_file_name,
            self.settings.dark_mode,
        ) {
            match self.project.create_file_on_disk(&new_file_str) {
                Ok(rel) => {
                    let initial_text = self.project.read_source_text(&rel).unwrap_or_default();
                    if self.collab_ui.is_active {
                        self.collab_session.send(UiToSessionCmd::LocalFileCreated {
                            rel_path: rel.clone(),
                            initial_text,
                        });
                    }
                    self.status_notice = format!(
                        "Created '{}' on disk",
                        self.project.root_dir.join(&rel).display()
                    );
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice = format!("Create file failed: {}", e);
                }
            }
        }

        if let Some((old_rel, new_str)) = crate::ui::dialogs::file_modals::render_rename_file_modal(
            ctx,
            &mut self.rename_target_file,
            &mut self.rename_file_input,
            self.settings.dark_mode,
        ) {
            match self.project.rename_file_on_disk(&old_rel, &new_str) {
                Ok(new_rel) => {
                    if self.collab_ui.is_active {
                        self.collab_session.send(UiToSessionCmd::LocalFileRenamed {
                            old_rel_path: old_rel.clone(),
                            new_rel_path: new_rel.clone(),
                        });
                    }
                    self.status_notice =
                        format!("Renamed '{}' -> '{}'", old_rel.display(), new_rel.display());
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice = format!("Rename failed: {}", e);
                }
            }
        }
    }

    /// Renders the Settings sub-window (`File > Settings`) as a real OS sub-window.
    fn render_settings_subwindow(&mut self, ctx: &Context) {
        let output = crate::ui::dialogs::settings::render_settings_dialog(
            ctx,
            &mut self.show_settings_window,
            &mut self.settings,
            &mut self.settings_tab,
            &self.project.root_dir,
        );

        if output.open_folder_requested {
            self.open_folder_dialog(ctx);
        }
        if output.theme_changed {
            Self::apply_theme(ctx, self.settings.dark_mode);
        }
        if output.rescan_needed {
            let _ = self.project.rescan_tree();
        }
        if output.dpi_changed {
            self.trigger_compile_now(ctx);
        }
    }

    fn render_bottom_status_bar(&mut self, ctx: &Context) {
        let palette = ThemePalette::for_mode(self.settings.dark_mode);
        let bar_bg = palette.chrome_sidebar;
        let border_col = palette.panel_border;

        let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
        let warn_count = self.diagnostics.iter().filter(|d| !d.is_error).count();

        TopBottomPanel::bottom("bottom_status_bar")
            .frame(
                Frame::new()
                    .fill(bar_bg)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::symmetric(12, 5))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (dot_color, status_text) =
                        if self.compile_in_progress || self.pending_compile {
                            (palette.syn_operator, "Compiling...".to_string())
                        } else if error_count > 0 {
                            (palette.syn_number, format!("{} Error(s)", error_count))
                        } else {
                            (
                                Color32::from_rgb(0x44, 0x8C, 0x6B),
                                format!("Compiled ({:.1} ms)", self.compile_duration_ms),
                            )
                        };

                    ui.label(RichText::new(status_text).color(dot_color).strong().small());

                    if warn_count > 0 {
                        ui.separator();
                        ui.label(
                            RichText::new(format!("{} Warning(s)", warn_count))
                                .color(palette.syn_operator)
                                .small(),
                        );
                    }

                    ui.separator();
                    ui.label(
                        RichText::new(format!("Main: {}", self.project.main_file.display()))
                            .color(palette.text_primary)
                            .small(),
                    );

                    ui.separator();
                    ui.label(
                        RichText::new(format!("Pages: {}", self.page_textures.len()))
                            .color(palette.text_primary)
                            .small(),
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
                                .color(palette.syn_keyword),
                        );
                        ui.separator();
                    }
                    ui.label(
                        RichText::new(&self.status_notice)
                            .small()
                            .color(palette.text_muted),
                    );

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
                                .monospace()
                                .color(palette.text_muted),
                            );
                        }
                    });
                });
            });
    }

    pub fn extract_typst_markup_plain_text(node: &SyntaxNode) -> String {
        crate::core::outline::extract_typst_markup_plain_text(node)
    }

    pub fn extract_typst_ast_outline(
        text: &str,
        current_file: &Path,
        project: Option<&DiskProject>,
        visited: &mut Vec<PathBuf>,
    ) -> Vec<DocumentOutlineItem> {
        let resolver = project.map(|p| p as &dyn crate::core::outline::SourceResolver);
        crate::core::outline::extract_typst_ast_outline(text, current_file, resolver, visited)
    }

    pub fn assign_hierarchical_numbers(items: &mut [DocumentOutlineItem]) {
        crate::core::outline::assign_hierarchical_numbers(items);
    }

    pub fn build_outline_tree(
        items: &[DocumentOutlineItem],
        active_idx: Option<usize>,
    ) -> Vec<OutlineTreeNode> {
        crate::core::outline::build_outline_tree(items, active_idx)
    }

    pub fn extract_document_outline(
        text: &str,
        kind: FileKind,
        current_file: &Path,
        project: Option<&DiskProject>,
        compiled_outline: &[CompiledOutlineItem],
    ) -> Vec<DocumentOutlineItem> {
        let resolver = project.map(|p| p as &dyn crate::core::outline::SourceResolver);
        crate::core::outline::extract_document_outline(
            text,
            kind,
            current_file,
            resolver,
            compiled_outline,
        )
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
                let (guide_rect, _) =
                    ui.allocate_exact_size(Vec2::new(indent_w, 18.0), Sense::hover());
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
                        "Collapse subheadings"
                    } else {
                        "Expand subheadings"
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
                    "{} {} — {}\nFile: {} (Line {})\nClick to open & jump cursor here",
                    "=".repeat(node.item.level),
                    node.item.number_label,
                    node.item.title,
                    node.item.file.display(),
                    node.item.line_number
                )
            } else {
                format!(
                    "{} {} — {}\nLine {}\nClick to jump cursor here",
                    "=".repeat(node.item.level),
                    node.item.number_label,
                    node.item.title,
                    node.item.line_number
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
                Self::render_outline_tree_node(
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

    fn render_left_file_panel(&mut self, ctx: &Context) {
        if !self.show_sidebar {
            return;
        }

        let palette = ThemePalette::for_mode(self.settings.dark_mode);
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
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("DISK PROJECT TREE")
                            .size(11.5)
                            .strong()
                            .color(palette.text_muted),
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

                let total_sidebar_h = ui.available_height();
                let tree_max_h = (total_sidebar_h * 0.42).max(125.0);

                ScrollArea::vertical()
                    .id_salt("file_tree_scroll")
                    .max_height(tree_max_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        CollapsingHeader::new(
                            RichText::new(project_title)
                                .strong()
                                .color(palette.text_primary),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            for node in &tree_snapshot {
                                Self::render_fs_tree_node(
                                    ui,
                                    node,
                                    &self.project,
                                    &palette,
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
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Document Outline Hierarchy Panel for the currently open text editor file
                let outline_items = self
                    .project
                    .active_buffer()
                    .and_then(|buf| {
                        buf.text.as_deref().map(|txt| {
                            Self::extract_document_outline(
                                txt,
                                buf.kind,
                                &buf.rel_path,
                                Some(&self.project),
                                &self.compiled_outline,
                            )
                        })
                    })
                    .unwrap_or_default();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("DOCUMENT OUTLINE")
                            .size(11.5)
                            .strong()
                            .color(palette.text_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} heading(s)", outline_items.len()))
                                .size(10.5)
                                .monospace()
                                .color(palette.text_muted),
                        );
                    });
                });

                ui.add_space(4.0);

                let current_cursor_char = self
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

                let outline_tree = Self::build_outline_tree(&outline_items, active_outline_idx);
                let mut navigate_to_item: Option<DocumentOutlineItem> = None;

                Frame::new()
                    .fill(palette.editor_canvas)
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(8, 8))
                    .stroke(Stroke::new(1.0_f32, border_col))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("document_outline_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if outline_tree.is_empty() {
                                    ui.label(
                                        RichText::new(
                                            "No Typst headings in active file.\nUse `= Heading`, `== Section`, or `=== Subsection` to build the hierarchy.",
                                        )
                                        .small()
                                        .color(palette.text_muted),
                                    );
                                } else {
                                    let root_len = outline_tree.len();
                                    for (idx, root_node) in outline_tree.iter().enumerate() {
                                        Self::render_outline_tree_node(
                                            ui,
                                            root_node,
                                            0,
                                            idx + 1 == root_len,
                                            &active_path,
                                            &palette,
                                            &mut navigate_to_item,
                                        );
                                    }
                                }
                            });
                    });

                if let Some(target_item) = navigate_to_item {
                    if target_item.file != self.project.active_file {
                        let _ = self.project.open_file(&target_item.file);
                    }
                    let sel = CharSelection {
                        primary_char: target_item.char_offset,
                        secondary_char: target_item.char_offset,
                    };
                    self.pending_cursor_restore = Some(sel);
                    self.pending_scroll_to_char = Some(target_item.char_offset);
                    self.last_editor_selection = Some(sel);
                    self.inspect_asset_mode = false;
                    self.status_notice = format!(
                        "Navigated to '{} {}' ({}:{})",
                        target_item.number_label,
                        target_item.title,
                        target_item.file.display(),
                        target_item.line_number
                    );
                }

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
                            Self::render_fs_tree_node(
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
                    let label_text =
                        format!("{} {}{}{}", kind.icon(), name, main_badge, dirty_mark);
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
        let dark_mode = self.settings.dark_mode;
        let palette = ThemePalette::for_mode(dark_mode);
        let border_col = palette.panel_border;
        let active_path = self.project.active_file.clone();
        let active_kind = FileKind::from_path(&active_path);

        // Docked Chrome Header for Open File Tabs & Quick Formatting Toolbar
        Frame::new()
            .fill(palette.chrome_sidebar)
            .stroke(Stroke::new(1.0_f32, border_col))
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                // 1. Open File Tabs Bar
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let tabs = self.project.open_tabs.clone();
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
                        let tab_text = RichText::new(tab_label).size(12.0).color(if is_active {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        });

                        if ui
                            .selectable_label(is_active, tab_text)
                            .on_hover_text(
                                self.project.root_dir.join(tab_path).display().to_string(),
                            )
                            .clicked()
                        {
                            select_tab = Some(tab_path.clone());
                        }
                        if tabs.len() > 1
                            && ui
                                .small_button(RichText::new("×").color(palette.text_muted))
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
                        if ui
                            .selectable_label(self.settings.word_wrap, "↩ Wrap")
                            .on_hover_text(
                                "Toggle automatic line wrapping at editor panel width (also in File > Settings)",
                            )
                            .clicked()
                        {
                            self.settings.word_wrap = !self.settings.word_wrap;
                        }
                    });
                });
            });

        // 3. Editor Area or Asset Inspector
        let font_size = self.settings.editor_font_size;
        let word_wrap = self.settings.word_wrap;
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
        let scroll_to_char = self.pending_scroll_to_char.take();

        let Some(file) = self.project.active_buffer_mut() else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a file from the Disk Project Tree.");
            });
            return;
        };

        if file.text.is_none() || inspect_asset_mode {
            let mut exit_inspect = false;
            Frame::new()
                .fill(palette.editor_canvas)
                .inner_margin(Margin::same(16))
                .corner_radius(CornerRadius::ZERO)
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
        let file_kind = file.kind;
        let mut text_changed = false;
        let mut edited_text_snapshot: Option<String> = None;
        let mut observed_cursor: Option<CharSelection> = None;
        let mut context_action: Option<EditorContextAction> =
            self.pending_editor_context_action.take();
        let prev_selection = self.last_editor_selection;

        let editor_bg = palette.editor_canvas;
        let gutter_fg = palette.text_muted;

        Frame::new()
            .fill(editor_bg)
            .stroke(Stroke::new(1.0_f32, border_col))
            .corner_radius(CornerRadius::ZERO)
            .show(ui, |ui| {
                ScrollArea::new([!word_wrap, true])
                    .id_salt("code_editor_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let text_ref = file.text.as_mut().unwrap();
                        let line_count = text_ref.lines().count().max(1);
                        let digits = line_count.to_string().len().max(3);
                        let gutter_w = if show_line_numbers {
                            (digits as f32 * (font_size * 0.62) + 18.0).max(40.0)
                        } else {
                            0.0
                        };

                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;

                            let gutter_x_min = ui.cursor().min.x;
                            if show_line_numbers {
                                ui.add_space(gutter_w);
                            }
                            let gutter_x_max = gutter_x_min + gutter_w;

                            Frame::new()
                                .fill(editor_bg)
                                .inner_margin(Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    let avail_editor_size = ui.available_size();
                                    // Reserve 22px on the right when word_wrap is enabled so the end-of-line
                                    // wrap indicator (`↩`) is always clearly visible inside the editor panel.
                                    let target_wrap_width = if word_wrap {
                                        (avail_editor_size.x - 22.0).max(80.0)
                                    } else {
                                        f32::INFINITY
                                    };

                                    let mut layouter =
                                        |ui: &Ui, string: &str, _wrap_width: f32| {
                                            let mut job = if is_typst {
                                                highlight_typst_code(string, dark_mode, font_size)
                                            } else {
                                                LayoutJob::simple(
                                                    string.to_owned(),
                                                    FontId::new(font_size, FontFamily::Monospace),
                                                    palette.syn_default,
                                                    target_wrap_width,
                                                )
                                            };
                                            job.wrap.max_width = target_wrap_width;
                                            ui.fonts(|f| f.layout_job(job))
                                        };

                                    // Reserve shape slot before TextEdit so the Active Line Highlight
                                    // (#262530 dark / #EBF3FE light) renders behind the active cursor line
                                    let active_line_shape_idx =
                                        ui.painter().add(egui::Shape::Noop);

                                    let mut output = TextEdit::multiline(text_ref)
                                        .id_salt("typst_code_editor")
                                        .code_editor()
                                        .desired_width(if word_wrap {
                                            avail_editor_size.x
                                        } else {
                                            f32::INFINITY
                                        })
                                        .min_size(avail_editor_size)
                                        .desired_rows(34)
                                        .frame(false)
                                        .font(FontId::new(font_size, FontFamily::Monospace))
                                        .layouter(&mut layouter)
                                        .show(ui);

                                    // Restore StickyIndex-preserved cursor or Document Outline click navigation
                                    // BEFORE painting the Active Line Highlight so the highlight and cursor
                                    // jump immediately to the selected heading on the same frame.
                                    if let Some(sel) = restore_cursor {
                                        output.state.cursor.set_char_range(Some(
                                            CCursorRange::two(
                                                CCursor::new(sel.secondary_char),
                                                CCursor::new(sel.primary_char),
                                            ),
                                        ));
                                        output.state.clone().store(ctx, output.response.id);
                                    } else if ctx.input(|i| i.pointer.secondary_pressed())
                                        && output.response.hovered()
                                    {
                                        // Preserve existing text selection when right-clicking inside the editor
                                        if let Some(prev) = prev_selection {
                                            if prev.primary_char != prev.secondary_char {
                                                let restored = CCursorRange::two(
                                                    CCursor::new(prev.secondary_char),
                                                    CCursor::new(prev.primary_char),
                                                );
                                                output.state.cursor.set_char_range(Some(restored));
                                                output.state.clone().store(ctx, output.response.id);
                                            }
                                        }
                                    }

                                    // If navigated from the Document Outline panel, focus editor and scroll to target heading
                                    if let Some(target_char) = scroll_to_char {
                                        output.response.request_focus();
                                        let target_cursor =
                                            output.galley.from_ccursor(CCursor::new(target_char));
                                        let local_rect =
                                            output.galley.pos_from_cursor(&target_cursor);
                                        let screen_rect =
                                            local_rect.translate(output.galley_pos.to_vec2());
                                        ui.scroll_to_rect(
                                            screen_rect.expand2(Vec2::new(0.0, 56.0)),
                                            Some(Align::Center),
                                        );
                                        ctx.request_repaint();
                                    }

                                    // Paint aligned gutter line numbers and end-of-line wrap indicators (`↩`)
                                    // for every wrapped visual row in the galley.
                                    let total_rows = output.galley.rows.len();
                                    let mut logical_line = 1usize;
                                    let mut is_wrapped_continuation = false;
                                    let gutter_font = FontId::new(font_size, FontFamily::Monospace);
                                    let cont_font =
                                        FontId::new((font_size * 0.85).max(9.5), FontFamily::Monospace);
                                    let wrap_badge_font =
                                        FontId::new((font_size * 0.90).max(10.5), FontFamily::Monospace);

                                    for (row_idx, row) in output.galley.rows.iter().enumerate() {
                                        let row_top_y = output.galley_pos.y + row.min_y();
                                        let row_center_y =
                                            output.galley_pos.y + row.rect.center().y;

                                        if show_line_numbers {
                                            if !is_wrapped_continuation {
                                                ui.painter().text(
                                                    Pos2::new(gutter_x_max - 8.0, row_top_y),
                                                    Align2::RIGHT_TOP,
                                                    logical_line.to_string(),
                                                    gutter_font.clone(),
                                                    gutter_fg,
                                                );
                                            } else {
                                                ui.painter().text(
                                                    Pos2::new(gutter_x_max - 8.0, row_top_y),
                                                    Align2::RIGHT_TOP,
                                                    "↪",
                                                    cont_font.clone(),
                                                    gutter_fg.gamma_multiply(0.65),
                                                );
                                            }
                                        }

                                        let wrapped_to_next = word_wrap
                                            && !row.ends_with_newline
                                            && (row_idx + 1 < total_rows);
                                        if wrapped_to_next {
                                            let eol_x = (output.galley_pos.x
                                                + row.rect.max.x
                                                + 4.0)
                                                .min(output.response.rect.max.x - 12.0);
                                            ui.painter().text(
                                                Pos2::new(eol_x, row_center_y),
                                                Align2::LEFT_CENTER,
                                                "↩",
                                                wrap_badge_font.clone(),
                                                palette.syn_keyword,
                                            );
                                        }

                                        if row.ends_with_newline {
                                            logical_line += 1;
                                            is_wrapped_continuation = false;
                                        } else {
                                            is_wrapped_continuation = true;
                                        }
                                    }

                                    if show_line_numbers {
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(
                                                    gutter_x_max,
                                                    output.response.rect.min.y - 8.0,
                                                ),
                                                Pos2::new(
                                                    gutter_x_max,
                                                    output.response.rect.max.y + 8.0,
                                                ),
                                            ],
                                            Stroke::new(1.0_f32, border_col),
                                        );
                                    }

                                    let current_range = output
                                        .state
                                        .cursor
                                        .char_range()
                                        .or_else(|| output.cursor_range.map(|cr| cr.as_ccursor_range()));

                                    // Paint full-width Active Line Highlight (#262530 Dark / #EBF3FE Light)
                                    let active_ccursor = current_range
                                        .map(|r| r.primary)
                                        .unwrap_or_else(|| CCursor::new(0));
                                    let active_cursor_obj =
                                        output.galley.from_ccursor(active_ccursor);
                                    let cursor_local_rect =
                                        output.galley.pos_from_cursor(&active_cursor_obj);
                                    let line_min_y = output.galley_pos.y + cursor_local_rect.min.y;
                                    let line_max_y = output.galley_pos.y + cursor_local_rect.max.y;
                                    let full_line_rect = Rect::from_min_max(
                                        Pos2::new(output.response.rect.min.x - 10.0, line_min_y),
                                        Pos2::new(output.response.rect.max.x + 10.0, line_max_y),
                                    );
                                    ui.painter().set(
                                        active_line_shape_idx,
                                        egui::Shape::rect_filled(
                                            full_line_rect,
                                            CornerRadius::ZERO,
                                            palette.active_line_highlight,
                                        ),
                                    );
                                    let has_selection = current_range
                                        .map(|r| r.primary.index != r.secondary.index)
                                        .unwrap_or(false);
                                    let cur_undo_state =
                                        (current_range.unwrap_or_default(), text_ref.clone());
                                    let can_undo = output.state.undoer().has_undo(&cur_undo_state);
                                    let can_redo = output.state.undoer().has_redo(&cur_undo_state);

                                    // Right-click context menu inside the text editor
                                    output.response.context_menu(|ui| {
                                        ui.set_min_width(220.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "Editor — {}",
                                                active_path.display()
                                            ))
                                            .small()
                                            .strong()
                                            .weak(),
                                        );
                                        ui.separator();

                                        if ui
                                            .add_enabled(
                                                can_undo,
                                                egui::Button::new("Undo  (Ctrl+Z)"),
                                            )
                                            .clicked()
                                        {
                                            context_action = Some(EditorContextAction::Undo);
                                            ui.close_menu();
                                        }
                                        if ui
                                            .add_enabled(
                                                can_redo,
                                                egui::Button::new("Redo  (Ctrl+Y)"),
                                            )
                                            .clicked()
                                        {
                                            context_action = Some(EditorContextAction::Redo);
                                            ui.close_menu();
                                        }

                                        ui.separator();

                                        if ui
                                            .add_enabled(
                                                has_selection,
                                                egui::Button::new("Cut  (Ctrl+X)"),
                                            )
                                            .clicked()
                                        {
                                            context_action = Some(EditorContextAction::Cut);
                                            ui.close_menu();
                                        }
                                        if ui
                                            .add_enabled(
                                                has_selection,
                                                egui::Button::new("Copy  (Ctrl+C)"),
                                            )
                                            .clicked()
                                        {
                                            context_action = Some(EditorContextAction::Copy);
                                            ui.close_menu();
                                        }
                                        if ui.button("Paste  (Ctrl+V)").clicked() {
                                            context_action = Some(EditorContextAction::Paste);
                                            ui.close_menu();
                                        }
                                        if ui.button("Select All  (Ctrl+A)").clicked() {
                                            context_action = Some(EditorContextAction::SelectAll);
                                            ui.close_menu();
                                        }

                                        ui.separator();

                                        if ui.button("Bold  (*...*)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::WrapOrInsert(
                                                    "*bold*",
                                                    "*",
                                                    "*",
                                                ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Italic  (_..._)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::WrapOrInsert(
                                                    "_italic_",
                                                    "_",
                                                    "_",
                                                ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Inline Code  (`...`)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::WrapOrInsert(
                                                    "`code`",
                                                    "`",
                                                    "`",
                                                ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Inline Math  ($...$)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::WrapOrInsert(
                                                    "$x$",
                                                    "$",
                                                    "$",
                                                ));
                                            ui.close_menu();
                                        }

                                        ui.menu_button("Insert Typst Element", |ui| {
                                            ui.set_min_width(210.0);
                                            if ui.button("Section Heading (== ...)").clicked() {
                                                context_action =
                                                    Some(EditorContextAction::InsertSnippet(
                                                        "\n== New Section\n",
                                                    ));
                                                ui.close_menu();
                                            }
                                            if ui.button("Display Math Block ($ ... $)").clicked() {
                                                context_action =
                                                    Some(EditorContextAction::InsertSnippet(
                                                        "\n$ f(x) = integral_0^x e^(-t^2) dif t $\n",
                                                    ));
                                                ui.close_menu();
                                            }
                                            if ui.button("Formatted Table (#table)").clicked() {
                                                context_action =
                                                    Some(EditorContextAction::InsertSnippet(
                                                        "\n#table(\n  columns: (1fr, 1fr),\n  [*Column A*], [*Column B*],\n  [Item 1], [Value 1],\n)\n",
                                                    ));
                                                ui.close_menu();
                                            }
                                            if ui.button("Figure / Image (#figure)").clicked() {
                                                context_action =
                                                    Some(EditorContextAction::InsertSnippet(
                                                        "\n#figure(\n  image(\"assets/diagram.svg\", width: 80%),\n  caption: [Figure caption],\n)\n",
                                                    ));
                                                ui.close_menu();
                                            }
                                            if ui.button("Code Block (```rust ... ```)").clicked() {
                                                context_action =
                                                    Some(EditorContextAction::InsertSnippet(
                                                        "\n```rust\nfn main() {\n    println!(\"Hello, Typst!\");\n}\n```\n",
                                                    ));
                                                ui.close_menu();
                                            }
                                        });

                                        ui.separator();

                                        if ui.button("Save File  (Ctrl+S)").clicked() {
                                            context_action =
                                                Some(EditorContextAction::Menu(MenuAction::SaveFile));
                                            ui.close_menu();
                                        }
                                        if ui.button("Compile Document Now").clicked() {
                                            context_action = Some(EditorContextAction::Menu(
                                                MenuAction::CompileNow,
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("Export PDF...  (Ctrl+E)").clicked() {
                                            context_action = Some(EditorContextAction::Menu(
                                                MenuAction::ExportPdf,
                                            ));
                                            ui.close_menu();
                                        }
                                        if file_kind == FileKind::Typst
                                            && ui.button("Set as Main Entrypoint").clicked()
                                        {
                                            context_action =
                                                Some(EditorContextAction::SetActiveAsMain);
                                            ui.close_menu();
                                        }
                                    });

                                    // Apply any triggered editor context menu or Edit menu action
                                    if let Some(ref act) = context_action {
                                        let total_chars = text_ref.chars().count();
                                        let (start_char, end_char, cursor_char) =
                                            if let Some(r) = current_range {
                                                let a = r.primary.index.min(total_chars);
                                                let b = r.secondary.index.min(total_chars);
                                                (a.min(b), a.max(b), a)
                                            } else {
                                                (total_chars, total_chars, total_chars)
                                            };

                                        let char_to_byte = |s: &str, char_idx: usize| -> usize {
                                            s.char_indices()
                                                .nth(char_idx)
                                                .map(|(i, _)| i)
                                                .unwrap_or(s.len())
                                        };

                                        let ccursor_range = current_range.unwrap_or_default();

                                        match *act {
                                            EditorContextAction::Undo => {
                                                let mut undoer = output.state.undoer();
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(ccursor_range, text_ref.clone()),
                                                );
                                                if let Some((undo_range, undo_txt)) =
                                                    undoer.undo(&(
                                                        ccursor_range,
                                                        text_ref.clone(),
                                                    ))
                                                {
                                                    *text_ref = undo_txt.clone();
                                                    output.state.cursor.set_char_range(Some(*undo_range));
                                                    output.state.set_undoer(undoer);
                                                    output
                                                        .state
                                                        .clone()
                                                        .store(ctx, output.response.id);
                                                    text_changed = true;
                                                    edited_text_snapshot = Some(text_ref.clone());
                                                }
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::Redo => {
                                                let mut undoer = output.state.undoer();
                                                if let Some((redo_range, redo_txt)) =
                                                    undoer.redo(&(
                                                        ccursor_range,
                                                        text_ref.clone(),
                                                    ))
                                                {
                                                    *text_ref = redo_txt.clone();
                                                    output.state.cursor.set_char_range(Some(*redo_range));
                                                    output.state.set_undoer(undoer);
                                                    output
                                                        .state
                                                        .clone()
                                                        .store(ctx, output.response.id);
                                                    text_changed = true;
                                                    edited_text_snapshot = Some(text_ref.clone());
                                                }
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::Copy => {
                                                if start_char < end_char {
                                                    let start_b =
                                                        char_to_byte(text_ref, start_char);
                                                    let end_b = char_to_byte(text_ref, end_char);
                                                    let selected_str =
                                                        text_ref[start_b..end_b].to_string();
                                                    ctx.copy_text(selected_str.clone());
                                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                                        let _ = cb.set_text(selected_str.clone());
                                                    }
                                                    self.clipboard_fallback = selected_str;
                                                }
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::Cut => {
                                                if start_char < end_char {
                                                    let mut undoer = output.state.undoer();
                                                    undoer.feed_state(
                                                        ctx.input(|i| i.time),
                                                        &(ccursor_range, text_ref.clone()),
                                                    );
                                                    let start_b =
                                                        char_to_byte(text_ref, start_char);
                                                    let end_b = char_to_byte(text_ref, end_char);
                                                    let selected_str =
                                                        text_ref[start_b..end_b].to_string();
                                                    ctx.copy_text(selected_str.clone());
                                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                                        let _ = cb.set_text(selected_str.clone());
                                                    }
                                                    self.clipboard_fallback = selected_str;

                                                    text_ref.replace_range(start_b..end_b, "");
                                                    let new_range =
                                                        CCursorRange::one(CCursor::new(start_char));
                                                    output.state.cursor.set_char_range(Some(new_range));
                                                    undoer.feed_state(
                                                        ctx.input(|i| i.time),
                                                        &(new_range, text_ref.clone()),
                                                    );
                                                    output.state.set_undoer(undoer);
                                                    output
                                                        .state
                                                        .clone()
                                                        .store(ctx, output.response.id);
                                                    text_changed = true;
                                                    edited_text_snapshot = Some(text_ref.clone());
                                                }
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::Paste => {
                                                let clip_text = arboard::Clipboard::new()
                                                    .and_then(|mut cb| cb.get_text())
                                                    .ok()
                                                    .filter(|s| !s.is_empty())
                                                    .unwrap_or_else(|| {
                                                        self.clipboard_fallback.clone()
                                                    });
                                                if !clip_text.is_empty() {
                                                    let mut undoer = output.state.undoer();
                                                    undoer.feed_state(
                                                        ctx.input(|i| i.time),
                                                        &(ccursor_range, text_ref.clone()),
                                                    );
                                                    let start_b =
                                                        char_to_byte(text_ref, start_char);
                                                    let end_b = char_to_byte(text_ref, end_char);
                                                    text_ref
                                                        .replace_range(start_b..end_b, &clip_text);
                                                    let pasted_chars = clip_text.chars().count();
                                                    let new_pos = start_char + pasted_chars;
                                                    let new_range =
                                                        CCursorRange::one(CCursor::new(new_pos));
                                                    output
                                                        .state
                                                        .cursor
                                                        .set_char_range(Some(new_range));
                                                    undoer.feed_state(
                                                        ctx.input(|i| i.time),
                                                        &(new_range, text_ref.clone()),
                                                    );
                                                    output.state.set_undoer(undoer);
                                                    output
                                                        .state
                                                        .clone()
                                                        .store(ctx, output.response.id);
                                                    text_changed = true;
                                                    edited_text_snapshot = Some(text_ref.clone());
                                                }
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::SelectAll => {
                                                let full_len = text_ref.chars().count();
                                                let all_range = CCursorRange::two(
                                                    CCursor::new(0),
                                                    CCursor::new(full_len),
                                                );
                                                output.state.cursor.set_char_range(Some(all_range));
                                                output
                                                    .state
                                                    .clone()
                                                    .store(ctx, output.response.id);
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::WrapOrInsert(
                                                default_snippet,
                                                prefix,
                                                suffix,
                                            ) => {
                                                let mut undoer = output.state.undoer();
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(ccursor_range, text_ref.clone()),
                                                );
                                                if start_char < end_char {
                                                    let start_b =
                                                        char_to_byte(text_ref, start_char);
                                                    let end_b = char_to_byte(text_ref, end_char);
                                                    let inner = text_ref[start_b..end_b].to_string();
                                                    let wrapped =
                                                        format!("{}{}{}", prefix, inner, suffix);
                                                    let wrapped_chars = wrapped.chars().count();
                                                    text_ref
                                                        .replace_range(start_b..end_b, &wrapped);
                                                    let new_range = CCursorRange::two(
                                                        CCursor::new(start_char),
                                                        CCursor::new(start_char + wrapped_chars),
                                                    );
                                                    output
                                                        .state
                                                        .cursor
                                                        .set_char_range(Some(new_range));
                                                } else {
                                                    let ins_b = char_to_byte(text_ref, cursor_char);
                                                    text_ref.insert_str(ins_b, default_snippet);
                                                    let ins_chars = default_snippet.chars().count();
                                                    let new_range = CCursorRange::one(CCursor::new(
                                                        cursor_char + ins_chars,
                                                    ));
                                                    output
                                                        .state
                                                        .cursor
                                                        .set_char_range(Some(new_range));
                                                }
                                                output.state.set_undoer(undoer);
                                                output
                                                    .state
                                                    .clone()
                                                    .store(ctx, output.response.id);
                                                text_changed = true;
                                                edited_text_snapshot = Some(text_ref.clone());
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::InsertSnippet(snippet) => {
                                                let mut undoer = output.state.undoer();
                                                undoer.feed_state(
                                                    ctx.input(|i| i.time),
                                                    &(ccursor_range, text_ref.clone()),
                                                );
                                                let start_b = char_to_byte(text_ref, start_char);
                                                let end_b = char_to_byte(text_ref, end_char);
                                                text_ref.replace_range(start_b..end_b, snippet);
                                                let ins_chars = snippet.chars().count();
                                                let new_range = CCursorRange::one(CCursor::new(
                                                    start_char + ins_chars,
                                                ));
                                                output.state.cursor.set_char_range(Some(new_range));
                                                output.state.set_undoer(undoer);
                                                output
                                                    .state
                                                    .clone()
                                                    .store(ctx, output.response.id);
                                                text_changed = true;
                                                edited_text_snapshot = Some(text_ref.clone());
                                                output.response.request_focus();
                                            }
                                            EditorContextAction::Menu(_)
                                            | EditorContextAction::SetActiveAsMain => {}
                                        }
                                    }

                                    if let Some(range) = output
                                        .state
                                        .cursor
                                        .char_range()
                                        .or_else(|| output.cursor_range.map(|cr| cr.as_ccursor_range()))
                                    {
                                        observed_cursor = Some(CharSelection {
                                            primary_char: range.primary.index,
                                            secondary_char: range.secondary.index,
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

        if let Some(sel) = observed_cursor {
            self.last_editor_selection = Some(sel);
        }

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

        if let Some(act) = context_action {
            match act {
                EditorContextAction::Menu(menu_act) => {
                    self.dispatch_menu_action(ctx, menu_act);
                }
                EditorContextAction::SetActiveAsMain => {
                    if file_kind == FileKind::Typst {
                        self.project.set_main_file(active_path.clone());
                        self.status_notice =
                            format!("Set '{}' as Main Entrypoint", active_path.display());
                        self.trigger_compile_now(ctx);
                    }
                }
                _ => {}
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
        let palette = ThemePalette::for_mode(self.settings.dark_mode);
        let canvas_bg = palette.preview_workspace;
        let border_col = palette.panel_border;

        Frame::new()
            .fill(palette.chrome_sidebar)
            .stroke(Stroke::new(1.0_f32, border_col))
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("LIVE DOCUMENT PREVIEW")
                            .size(11.5)
                            .strong()
                            .color(palette.text_muted),
                    );

                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} Page(s)", self.page_textures.len()))
                            .size(12.0)
                            .strong()
                            .color(palette.text_primary),
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
                                .monospace()
                                .color(palette.text_muted),
                        );
                    });
                });
            });

        let has_diagnostics = !self.diagnostics.is_empty();
        let available_height = ui.available_height();
        let banner_height = if has_diagnostics {
            (available_height * 0.28).clamp(110.0, 195.0)
        } else {
            0.0
        };
        let preview_height = (available_height - banner_height).max(120.0);

        ui.allocate_ui(Vec2::new(ui.available_width(), preview_height), |ui| {
            Frame::new()
                .fill(canvas_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::ZERO)
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
                                            .color(palette.text_muted),
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
                                        // Subtle layered depth (1px border + minimal soft shadow)
                                        let shadow_rect =
                                            rect.translate(Vec2::new(0.0, 2.0)).expand(1.0);
                                        painter.rect_filled(
                                            shadow_rect,
                                            CornerRadius::ZERO,
                                            Color32::from_black_alpha(if self.settings.dark_mode {
                                                45
                                            } else {
                                                22
                                            }),
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
                                            Stroke::new(1.0_f32, border_col),
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
                                            .color(palette.text_muted),
                                        );
                                        ui.add_space(18.0);
                                    }
                                }
                            });
                        });
                });
        });

        if has_diagnostics {
            let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
            let banner_bg = palette.chrome_sidebar;
            let banner_border = if error_count > 0 {
                palette.syn_number
            } else {
                palette.syn_operator
            };

            let mut file_to_open: Option<PathBuf> = None;

            Frame::new()
                .fill(banner_bg)
                .stroke(Stroke::new(1.0_f32, banner_border))
                .corner_radius(CornerRadius::ZERO)
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
                                        palette.syn_number
                                    } else {
                                        palette.syn_operator
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
                                    ui.label(
                                        RichText::new(&diag.message)
                                            .strong()
                                            .size(12.0)
                                            .color(palette.text_primary),
                                    );
                                });
                                for hint in &diag.hints {
                                    ui.label(
                                        RichText::new(format!("  Hint: {}", hint))
                                            .italics()
                                            .size(11.5)
                                            .color(palette.text_muted),
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

        // Desktop Menu Bar on Linux & Windows (on macOS, menu bar is part of the OS at the top of the screen)
        #[cfg(not(target_os = "macos"))]
        self.render_desktop_menu_bar(ctx);

        // Bottom Status Bar
        self.render_bottom_status_bar(ctx);

        // Collapsible Left File Overview Panel (Disk Project Tree)
        self.render_left_file_panel(ctx);

        // Main Split View (Code Editor Left + Live Preview Right)
        let palette = ThemePalette::for_mode(self.settings.dark_mode);
        let central_bg = palette.chrome_sidebar;

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(central_bg)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                let total_rect = ui.available_rect_before_wrap();
                let spacing = 4.0;
                let full_h = total_rect.height();
                let avail_w = (total_rect.width() - spacing).max(200.0);
                let min_pane_w = 180.0_f32.min(avail_w * 0.4);
                let max_pane_w = (avail_w - min_pane_w).max(min_pane_w);
                let left_w = (avail_w * self.editor_split_ratio).clamp(min_pane_w, max_pane_w);

                let left_rect = Rect::from_min_size(total_rect.min, Vec2::new(left_w, full_h));
                let div_x = total_rect.min.x + left_w + spacing * 0.5;
                let right_rect = Rect::from_min_size(
                    Pos2::new(total_rect.min.x + left_w + spacing, total_rect.min.y),
                    Vec2::new((avail_w - left_w).max(min_pane_w), full_h),
                );

                // Left Pane: Code Editor
                let mut left_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(left_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                self.render_editor_pane(ctx, &mut left_ui);

                // Right Pane: Live Document Preview
                let mut right_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(right_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                self.render_preview_pane(ctx, &mut right_ui);

                // Interactive Resizable Vertical Split Divider between Editor and Live Preview
                let divider_hit_rect = Rect::from_center_size(
                    Pos2::new(div_x, total_rect.center().y),
                    Vec2::new(10.0, full_h),
                );
                let div_resp = ui
                    .interact(
                        divider_hit_rect,
                        ui.id().with("editor_preview_split_divider"),
                        Sense::click_and_drag(),
                    )
                    .on_hover_text(
                        "Drag to resize Code Editor and Live Preview panels (double-click to reset 50/50)",
                    );

                if div_resp.dragged() {
                    if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                        let new_ratio = (pointer_pos.x - total_rect.min.x) / avail_w.max(1.0);
                        self.editor_split_ratio = new_ratio.clamp(0.15, 0.85);
                        ctx.request_repaint();
                    }
                }
                if div_resp.double_clicked() {
                    self.editor_split_ratio = 0.5;
                }

                let div_active = div_resp.hovered() || div_resp.dragged();
                if div_active {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }

                let line_col = if div_active {
                    palette.selection_stroke
                } else {
                    palette.panel_border
                };
                let line_width = if div_active { 2.0_f32 } else { 1.0_f32 };

                ui.painter().line_segment(
                    [
                        Pos2::new(div_x, total_rect.min.y),
                        Pos2::new(div_x, total_rect.max.y),
                    ],
                    Stroke::new(line_width, line_col),
                );

                // Subtle center resize handle pill on the divider seam
                let handle_rect = Rect::from_center_size(
                    Pos2::new(div_x, total_rect.center().y),
                    Vec2::new(4.0, 28.0),
                );
                ui.painter().rect_filled(
                    handle_rect,
                    CornerRadius::same(2),
                    if div_active {
                        palette.selection_stroke
                    } else {
                        palette.text_muted.gamma_multiply(0.45)
                    },
                );

                ui.advance_cursor_after_rect(total_rect);
            });

        // Native OS Sub-Windows (Settings, Export PDF, New/Rename File, P2P Collaboration)
        self.render_settings_subwindow(ctx);
        self.render_export_pdf_dialog(ctx);
        self.render_file_dialogs_subwindows(ctx);

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
