use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::{
    Align, Context, CornerRadius, CursorIcon, Frame, Key, Layout, Margin, Modifiers, Pos2, Rect,
    Sense, Stroke, Ui, Vec2,
};

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::CharSelection;
use crate::collab::session::{CollabSessionHandle, SessionToUiEvent, UiToSessionCmd};
use crate::collab::ui::{render_collaboration_window, CollabUiState};
use crate::compiler::{
    compile_project, export_pdf_bytes_with_config, upload_page_textures, CompileSnapshot,
    CompiledOutlineItem, DiagnosticItem, PdfExportConfig, RenderedPageData, RenderedPageTexture,
};
use crate::highlight::ThemePalette;
use crate::project::{ensure_sample_project_on_disk, DiskProject, FileKind};
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
pub use crate::ui::components::EditorContextAction;

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
        let output = crate::ui::components::render_desktop_menu_bar(
            ctx,
            crate::ui::components::MenuBarParams {
                dark_mode: self.settings.dark_mode,
                show_sidebar: self.show_sidebar,
                word_wrap: self.settings.word_wrap,
                project_root: &self.project.root_dir,
                active_file: &self.project.active_file,
            },
        );

        if let Some(act) = output.editor_action {
            self.pending_editor_context_action = Some(act);
        }

        if let Some(snippet) = output.snippet_to_insert {
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

        if let Some(act) = output.pending_action {
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
        let active_buf_info =
            self.project
                .active_buffer()
                .map(|b| crate::ui::components::StatusBarBufferInfo {
                    rel_path: &b.rel_path,
                    dirty: b.dirty,
                    line_count: b.text.as_ref().map(|t| t.lines().count()).unwrap_or(0),
                    size_bytes: b.size_bytes,
                });

        let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
        let warn_count = self.diagnostics.iter().filter(|d| !d.is_error).count();

        crate::ui::components::render_status_bar(
            ctx,
            crate::ui::components::StatusBarParams {
                dark_mode: self.settings.dark_mode,
                compile_in_progress: self.compile_in_progress,
                pending_compile: self.pending_compile,
                error_count,
                warn_count,
                compile_duration_ms: self.compile_duration_ms,
                main_file: &self.project.main_file,
                page_count: self.page_textures.len(),
                collab_active: self.collab_ui.is_active,
                collab_is_host: self.collab_ui.is_host,
                collab_peer_count: self.collab_ui.connected_peer_count(),
                status_notice: &self.status_notice,
                active_buffer: active_buf_info,
            },
        );
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

    fn render_left_file_panel(&mut self, ctx: &Context) {
        let output = crate::ui::components::render_sidebar(
            ctx,
            crate::ui::components::SidebarParams {
                dark_mode: self.settings.dark_mode,
                show_sidebar: self.show_sidebar,
                project: &self.project,
                remote_presences: &self.collab_ui.remote_presences,
                compiled_outline: &self.compiled_outline,
                last_editor_selection: self.last_editor_selection,
            },
        );

        if output.rescan_tree {
            let _ = self.project.rescan_tree();
        }
        if output.show_new_file_modal {
            self.show_new_file_input = true;
        }
        if let Some(target_item) = output.navigate_to_item {
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
        if let Some(rel_path) = output.clicked_path {
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
        if let Some(main_p) = output.new_main_path {
            self.project.set_main_file(main_p.clone());
            self.status_notice = format!("Set '{}' as main Typst file", main_p.display());
            self.trigger_compile_now(ctx);
        }
        if let Some(rel_to_rename) = output.rename_req {
            self.rename_file_input = rel_to_rename.to_string_lossy().to_string();
            self.rename_target_file = Some(rel_to_rename);
        }
        if let Some(rel_to_delete) = output.delete_req {
            match self.project.delete_file_on_disk(&rel_to_delete) {
                Ok(()) => {
                    if self.collab_ui.is_active {
                        self.collab_session.send(UiToSessionCmd::LocalFileDeleted {
                            rel_path: rel_to_delete.clone(),
                        });
                    }
                    self.status_notice = format!("Deleted '{}' from disk", rel_to_delete.display());
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice = format!("Delete failed: {}", e);
                }
            }
        }
    }

    fn render_editor_pane(&mut self, ctx: &Context, ui: &mut Ui) {
        let active_path = self.project.active_file.clone();
        let peers: Vec<PeerPresence> = self
            .collab_ui
            .peers_in_file(&active_path)
            .into_iter()
            .cloned()
            .collect();

        let output = crate::ui::components::render_editor_pane(
            ctx,
            ui,
            crate::ui::components::EditorPaneParams {
                dark_mode: self.settings.dark_mode,
                editor_font_size: &mut self.settings.editor_font_size,
                word_wrap: &mut self.settings.word_wrap,
                show_line_numbers: self.settings.show_line_numbers,
                syntax_highlighting: self.settings.syntax_highlighting,
                inspect_asset_mode: &mut self.inspect_asset_mode,
                clipboard_fallback: &mut self.clipboard_fallback,
                project: &mut self.project,
                pending_cursor_restore: self.pending_cursor_restore.take(),
                pending_scroll_to_char: self.pending_scroll_to_char.take(),
                last_editor_selection: self.last_editor_selection,
                pending_editor_context_action: self.pending_editor_context_action.take(),
                peers_in_active_file: &peers,
            },
        );

        if let Some(target_tab) = output.select_tab {
            let _ = self.project.open_file(&target_tab);
            if self.settings.auto_compile {
                self.trigger_compile_now(ctx);
            }
        }
        if let Some(target_tab) = output.close_tab {
            self.project.close_tab(&target_tab);
            if self.settings.auto_compile {
                self.trigger_compile_now(ctx);
            }
        }

        if let Some(sel) = output.observed_cursor {
            self.last_editor_selection = Some(sel);
        }

        if output.text_changed {
            let auto_save = self.settings.auto_save_on_edit;
            self.project.mark_active_modified(auto_save);
            if let (true, Some(new_text)) = (self.collab_ui.is_active, output.edited_text_snapshot)
            {
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

        if let Some(act) = output.context_action {
            match act {
                crate::ui::components::EditorContextAction::Menu(menu_act) => {
                    self.dispatch_menu_action(ctx, menu_act);
                }
                crate::ui::components::EditorContextAction::SetActiveAsMain => {
                    if FileKind::from_path(&active_path) == FileKind::Typst {
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
            if let Some(sel) = output.observed_cursor {
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
        if let Some(file_to_open) = crate::ui::components::render_preview_pane(
            ui,
            crate::ui::components::PreviewPaneParams {
                dark_mode: self.settings.dark_mode,
                zoom: &mut self.zoom,
                page_textures: &self.page_textures,
                diagnostics: &self.diagnostics,
            },
        ) {
            let _ = self.project.open_file(&file_to_open);
        }
    }

    fn handle_screenshot_lifecycle(&mut self, ctx: &Context) {
        crate::services::handle_screenshot_lifecycle(
            ctx,
            self.screenshot_path.as_deref(),
            &mut self.frame_count,
            &mut self.screenshot_requested,
        );
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
