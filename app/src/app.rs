//! Top-level application state orchestrator (`AzoTypeApp`), event loop, and channel dispatcher.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::{
    Align, Context, CornerRadius, Frame, Key, Layout, Margin, Modifiers, Pos2, Rect, Ui, Vec2,
};

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::CharSelection;
use crate::collab::session::{CollabSessionHandle, SessionToUiEvent, UiToSessionCmd};
use crate::collab::ui::{render_collaboration_window, CollabUiState};
use crate::compiler::{
    upload_page_textures, CompileSnapshot, CompiledOutlineItem, DiagnosticItem, PdfExportConfig,
    RenderedPageData, RenderedPageTexture,
};
use crate::engine::EngineHost;
use crate::highlight::ThemePalette;
use crate::project::{DiskProject, FileKind};
use crate::ui::components::PreviewSelectionState;
use crate::ui::theme::apply_theme;
use crate::world::SharedWorldResources;

pub use crate::core::settings::{IdeSettings, PreviewZoom, SettingsTab, ThemePreference};
#[cfg(target_os = "macos")]
use crate::platform::menu::MacOsMenuBar;
pub use crate::platform::menu::MenuAction;
pub use crate::ui::components::EditorContextAction;

pub enum IoEvent {
    FolderPicked(PathBuf),
    FileSavedAs(PathBuf),
    StatusMessage(String),
}

/// Backward-compatibility alias for [`AzoTypeApp`].
pub type TypstIdeApp = AzoTypeApp;

pub struct AzoTypeApp {
    pub language: String,
    pub engine_host: EngineHost,
    pub project: DiskProject,
    pub settings: IdeSettings,
    pub show_sidebar: bool,
    pub editor_split_ratio: f32,
    pub zoom: PreviewZoom,
    pub show_settings_window: bool,
    pub settings_tab: SettingsTab,
    pub show_export_pdf_dialog: bool,
    pub pdf_export_config: PdfExportConfig,
    pub export_pdf_error: Option<String>,
    pub collab_ui: CollabUiState,
    pub collab_session: CollabSessionHandle,
    pub pending_cursor_restore: Option<CharSelection>,
    pub pending_scroll_to_char: Option<usize>,
    pub last_sent_cursor: Option<(PathBuf, CharSelection)>,
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
    pub show_new_file_input: bool,
    pub new_file_name: String,
    pub rename_target_file: Option<PathBuf>,
    pub rename_file_input: String,
    pub inspect_asset_mode: bool,
    pub last_editor_selection: Option<CharSelection>,
    pub preview_selection: PreviewSelectionState,
    pub clipboard_fallback: String,
    pub pending_editor_context_action: Option<EditorContextAction>,
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
    pub last_persisted_state: crate::services::PersistedAppState,
}

impl AzoTypeApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_folder: Option<PathBuf>,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        let resources = Arc::new(SharedWorldResources::new());
        let mut persisted =
            crate::services::load_persisted_state(cc.storage, &rust_i18n::available_locales!());
        persisted.settings.dark_mode = crate::platform::resolve_effective_dark_mode(
            &cc.egui_ctx,
            persisted.settings.theme_preference,
        );
        let last_persisted_state = persisted.clone();
        let (language, settings, zoom) = (persisted.language, persisted.settings, persisted.zoom);
        rust_i18n::set_locale(&language);

        let project = initial_folder
            .filter(|f| f.is_dir())
            .and_then(|f| DiskProject::open_folder(&f, settings.show_hidden_files).ok())
            .unwrap_or_default();

        apply_theme(&cc.egui_ctx, settings.dark_mode);

        let (engine_host, initial_snapshot) = EngineHost::init(
            resources,
            &project,
            &settings.typst_version,
            settings.render_dpi_scale,
        );
        let page_textures = upload_page_textures(&cc.egui_ctx, &initial_snapshot.pages);

        let (io_tx, io_rx) = mpsc::channel();
        let (compile_tx, compile_rx) = mpsc::channel();
        let collab_session = CollabSessionHandle::spawn(Some(cc.egui_ctx.clone()));

        let status_notice = engine_host.fallback_notice.clone().unwrap_or_else(|| {
            if project.has_disk_backing() {
                rust_i18n::t!(
                    "app.status_opened_folder",
                    name = project.project_name.as_str(),
                    path = project.root_dir.display().to_string()
                )
                .to_string()
            } else {
                rust_i18n::t!("app.status_blank_ready").to_string()
            }
        });

        #[cfg(target_os = "macos")]
        let macos_menu = MacOsMenuBar::init();

        Self {
            language,
            engine_host,
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
            zoom,
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
            preview_selection: PreviewSelectionState::default(),
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
            last_persisted_state,
        }
    }

    pub fn set_language(&mut self, ctx: &Context, lang: String) {
        if self.language != lang {
            self.language = lang;
            rust_i18n::set_locale(&self.language);
            ctx.request_repaint();
        }
    }

    fn dispatch_menu_action(&mut self, ctx: &Context, action: MenuAction) {
        match action {
            MenuAction::NewFile => self.show_new_file_input = true,
            MenuAction::OpenFolder => self.open_folder_dialog(ctx),
            MenuAction::RescanFolder => {
                if let Err(e) = self.project.rescan_tree() {
                    self.status_notice =
                        rust_i18n::t!("app.status_rescan_failed", error = e.to_string())
                            .to_string();
                } else {
                    let p = self.project.root_dir.display().to_string();
                    self.status_notice =
                        rust_i18n::t!("app.status_rescanned", path = p).to_string();
                    self.trigger_compile_now(ctx);
                }
            }
            MenuAction::SaveFile => {
                if !self.project.has_disk_backing() {
                    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    let f = &self.project.active_file;
                    let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("main.typ");
                    let data = self.project.read_file_bytes(f).unwrap_or_default();
                    let tx = self.io_tx.clone();
                    let ctx_c = ctx.clone();
                    crate::platform::dialogs::spawn_save_file_dialog(
                        &start,
                        name,
                        "Typst File (*.typ)",
                        "typ",
                        data,
                        move |res| {
                            let _ = tx.send(match res {
                                Ok(p) => IoEvent::FileSavedAs(p),
                                Err(e) => IoEvent::StatusMessage(
                                    rust_i18n::t!("app.status_save_error", error = e.to_string())
                                        .to_string(),
                                ),
                            });
                        },
                        move || ctx_c.request_repaint(),
                    );
                } else {
                    self.status_notice = match self.project.save_active_file() {
                        Ok(p) => {
                            self.trigger_compile_now(ctx);
                            rust_i18n::t!("app.status_saved_file", path = p.display().to_string())
                                .to_string()
                        }
                        Err(e) => rust_i18n::t!("app.status_save_error", error = e.to_string())
                            .to_string(),
                    };
                }
            }
            MenuAction::SaveAll => {
                self.status_notice = if !self.project.has_disk_backing() {
                    rust_i18n::t!("app.status_unsaved_warning").to_string()
                } else {
                    match self.project.save_all_files() {
                        Ok(n) => {
                            self.trigger_compile_now(ctx);
                            let p = self.project.root_dir.display().to_string();
                            rust_i18n::t!("app.status_saved_all", count = n, path = p).to_string()
                        }
                        Err(e) => rust_i18n::t!("app.status_save_error", error = e.to_string())
                            .to_string(),
                    }
                };
            }
            MenuAction::ExportPdf => {
                self.show_export_pdf_dialog = true;
                self.export_pdf_error = None;
            }
            MenuAction::OpenSettings => self.show_settings_window = true,
            MenuAction::OpenCollab => self.collab_ui.show_window = true,
            MenuAction::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            MenuAction::ToggleWordWrap => self.settings.word_wrap = !self.settings.word_wrap,
            MenuAction::ToggleTheme => {
                let next_dark = !self.settings.dark_mode;
                self.settings.theme_preference = if next_dark {
                    ThemePreference::Dark
                } else {
                    ThemePreference::Light
                };
                self.settings.dark_mode = next_dark;
                apply_theme(ctx, next_dark);
            }
            MenuAction::ZoomIn => self.zoom.step_in(),
            MenuAction::ZoomOut => self.zoom.step_out(),
            MenuAction::ZoomReset => self.zoom.reset_100(),
            MenuAction::CompileNow => self.trigger_compile_now(ctx),
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn trigger_compile_now(&mut self, ctx: &Context) {
        self.pending_compile = false;
        if !self.compile_in_progress {
            self.compile_in_progress = true;
            self.engine_host.trigger_compile(
                &self.settings.typst_version,
                self.project.clone(),
                self.settings.render_dpi_scale,
                self.compile_tx.clone(),
                ctx.clone(),
            );
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
            self.preview_selection.clear();
        }
        if snapshot.compiled_ok {
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

        let (dl_notice, dl_recompile) = self
            .engine_host
            .poll_downloads(&mut self.settings.typst_version);
        if let Some(notice) = dl_notice {
            self.status_notice = notice;
        }
        if dl_recompile {
            self.trigger_compile_now(ctx);
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
                            self.preview_selection.clear();
                            self.status_notice = rust_i18n::t!(
                                "app.status_opened_folder",
                                name = name.as_str(),
                                path = root_str
                            )
                            .to_string();
                            self.trigger_compile_now(ctx);
                        }
                        Err(err) => {
                            self.status_notice = rust_i18n::t!(
                                "app.status_cannot_open_folder",
                                error = err.to_string()
                            )
                            .to_string();
                        }
                    }
                }
                IoEvent::FileSavedAs(path) => {
                    let _ = self.project.save_active_to_path(&path);
                    self.status_notice =
                        rust_i18n::t!("app.status_saved_file", path = path.display().to_string())
                            .to_string();
                    self.trigger_compile_now(ctx);
                }
                IoEvent::StatusMessage(msg) => self.status_notice = msg,
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
                    self.collab_ui
                        .on_session_started(is_host, ticket_str, local_endpoint_id);
                    if !files.is_empty() {
                        self.project.sync_from_crdt_files(&files);
                        need_compile = true;
                    }
                }
                SessionToUiEvent::SessionStopped { reason } => {
                    self.collab_ui.reset_session();
                    self.collab_ui.is_host = false;
                    self.collab_ui.status_banner = Some(reason.clone());
                    self.status_notice = reason;
                }
                SessionToUiEvent::PeerListChanged { peers } => self.collab_ui.peers = peers,
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
                    self.status_notice = format!("Collab error: {message}");
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
            let shortcuts = [
                (Modifiers::COMMAND, Key::Comma, MenuAction::OpenSettings),
                (Modifiers::COMMAND, Key::S, MenuAction::SaveFile),
                (Modifiers::COMMAND, Key::O, MenuAction::OpenFolder),
                (Modifiers::COMMAND, Key::N, MenuAction::NewFile),
                (Modifiers::COMMAND, Key::B, MenuAction::ToggleSidebar),
                (Modifiers::COMMAND, Key::E, MenuAction::ExportPdf),
                (Modifiers::NONE, Key::F5, MenuAction::RescanFolder),
            ];
            for (m, k, act) in shortcuts {
                if i.consume_key(m, k) {
                    triggered = Some(act);
                    return;
                }
            }
            if collab_active {
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                    || i.consume_key(Modifiers::COMMAND, Key::Y)
                {
                    self.collab_session.send(UiToSessionCmd::LocalRedo {
                        rel_path: active_rel.clone(),
                    });
                } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
                    self.collab_session.send(UiToSessionCmd::LocalUndo {
                        rel_path: active_rel.clone(),
                    });
                }
            }
        });
        if let Some(action) = triggered {
            self.dispatch_menu_action(ctx, action);
        }
    }

    fn open_folder_dialog(&self, ctx: &Context) {
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();
        let start = if self.project.has_disk_backing() {
            self.project.root_dir.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        crate::platform::dialogs::spawn_open_folder_dialog(
            &start,
            move |p| {
                let _ = tx.send(IoEvent::FolderPicked(p));
            },
            move || ctx_clone.request_repaint(),
        );
    }

    fn export_pdf_action(&mut self, ctx: &Context) {
        let _ = self.project.save_all_files();
        let snapshot = self.project.clone();
        let outcome = crate::services::run_pdf_export_flow(
            ctx,
            &self.engine_host.manager,
            &self.settings.typst_version,
            &snapshot,
            &self.pdf_export_config,
            self.io_tx.clone(),
            IoEvent::StatusMessage,
        );
        self.export_pdf_error = outcome.export_pdf_error;
        self.status_notice = outcome.status_notice;
        self.engine_host.fallback_notice = outcome.fallback_notice;
        if outcome.exported {
            self.show_export_pdf_dialog = false;
        }
        if let Some(diags) = outcome.diagnostics {
            self.diagnostics = diags;
        }
    }

    /// Renders the Export PDF dialog window for selecting PDF version and ISO archival standards.
    fn render_export_pdf_dialog(&mut self, ctx: &Context) {
        let entry_file = self.project.effective_compile_entrypoint();
        let (active_engine_label, fallback_notice) = self
            .engine_host
            .manager
            .active_engine_label_and_fallback(&self.settings.typst_version);
        if crate::ui::dialogs::export_pdf::render_export_pdf_dialog(
            ctx,
            &mut self.show_export_pdf_dialog,
            &mut self.pdf_export_config,
            &mut self.export_pdf_error,
            &entry_file,
            &active_engine_label,
            fallback_notice.as_deref(),
            self.settings.dark_mode,
        ) {
            self.export_pdf_action(ctx);
        }
    }

    fn on_active_text_modified(&mut self, ctx: &Context, new_text: Option<String>) {
        self.project
            .mark_active_modified(self.settings.auto_save_on_edit);
        if let (true, Some(t)) = (self.collab_ui.is_active, new_text) {
            self.collab_session.send(UiToSessionCmd::LocalTextEdited {
                rel_path: self.project.active_file.clone(),
                new_text: t,
            });
        }
        if self.settings.auto_compile {
            self.pending_compile = true;
            self.last_edit_time = ctx.input(|i| i.time);
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
            let updated = self.project.active_buffer_mut().and_then(|buf| {
                buf.text.as_mut().map(|text| {
                    text.push_str(snippet);
                    text.clone()
                })
            });
            self.on_active_text_modified(ctx, updated);
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
                    let disk_p = self.project.root_dir.join(&rel).display().to_string();
                    self.status_notice =
                        rust_i18n::t!("app.status_created_file", path = disk_p).to_string();
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice =
                        rust_i18n::t!("app.status_create_failed", error = e.to_string())
                            .to_string();
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
                    let old_s = old_rel.display().to_string();
                    let new_s = new_rel.display().to_string();
                    self.status_notice =
                        rust_i18n::t!("app.status_renamed_file", old = old_s, new = new_s)
                            .to_string();
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice =
                        rust_i18n::t!("app.status_rename_failed", error = e.to_string())
                            .to_string();
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
            &self.language,
            &mut self.settings_tab,
            &self.project.root_dir,
            &self.engine_host.manager,
        );

        if let Some(l) = output.language_changed {
            self.set_language(ctx, l);
        }
        if output.open_folder_requested {
            self.open_folder_dialog(ctx);
        }
        if output.theme_changed {
            apply_theme(ctx, self.settings.dark_mode);
        }
        if output.rescan_needed {
            let _ = self.project.rescan_tree();
        }
        let (eng_notice, eng_recompile) = self.engine_host.manager.handle_settings_actions(
            output.download_typst_version,
            output.remove_typst_version,
            output.engine_changed,
            &self.settings.typst_version,
            &mut self.engine_host.fallback_notice,
            &self.engine_host.tx,
            ctx,
        );
        if let Some(msg) = eng_notice {
            self.status_notice = msg;
        }
        if output.dpi_changed || eng_recompile {
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
        let (active_ver, fallback) = self
            .engine_host
            .manager
            .active_version_and_fallback(&self.settings.typst_version);
        let downloading_ver = self.engine_host.manager.downloading_version();
        let notice = self
            .engine_host
            .fallback_notice
            .as_deref()
            .unwrap_or(&self.status_notice);

        let status_out = crate::ui::components::render_status_bar(
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
                active_typst_version: &active_ver,
                downloading_typst_version: downloading_ver.as_deref(),
                typst_fallback_active: fallback.is_some(),
                collab_active: self.collab_ui.is_active,
                collab_is_host: self.collab_ui.is_host,
                collab_peer_count: self.collab_ui.connected_peer_count(),
                status_notice: notice,
                active_buffer: active_buf_info,
            },
        );
        if status_out.open_compiler_settings {
            self.show_settings_window = true;
            self.settings_tab = SettingsTab::CompilerAndPreview;
        }
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
            self.preview_selection.clear();
            let sel = CharSelection::caret(target_item.char_offset);
            self.navigate_editor_to_source(ctx, &target_item.file, sel, target_item.char_offset);
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
            let path_s = main_p.display().to_string();
            self.status_notice = rust_i18n::t!("app.status_set_main", path = path_s).to_string();
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
                    let del_s = rel_to_delete.display().to_string();
                    self.status_notice =
                        rust_i18n::t!("app.status_deleted_file", path = del_s).to_string();
                    self.trigger_compile_now(ctx);
                }
                Err(e) => {
                    self.status_notice =
                        rust_i18n::t!("app.status_delete_failed", error = e.to_string())
                            .to_string();
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

        if let Some(tab) = output.select_tab {
            self.preview_selection.clear();
            let _ = self.project.open_file(&tab);
            if self.settings.auto_compile {
                self.trigger_compile_now(ctx);
            }
        }
        if let Some(tab) = output.close_tab {
            self.preview_selection.clear();
            self.project.close_tab(&tab);
            if self.settings.auto_compile {
                self.trigger_compile_now(ctx);
            }
        }
        if output.user_interacted {
            self.preview_selection.clear();
        }

        if let Some(sel) = output.observed_cursor {
            self.last_editor_selection = Some(sel);
        }

        if output.text_changed {
            self.on_active_text_modified(ctx, output.edited_text_snapshot);
        }

        if let Some(act) = output.context_action {
            match act {
                crate::ui::components::EditorContextAction::Menu(menu_act) => {
                    self.dispatch_menu_action(ctx, menu_act);
                }
                crate::ui::components::EditorContextAction::SetActiveAsMain
                    if FileKind::from_path(&active_path) == FileKind::Typst =>
                {
                    self.project.set_main_file(active_path.clone());
                    let p_str = active_path.display().to_string();
                    self.status_notice =
                        rust_i18n::t!("app.status_set_main", path = p_str).to_string();
                    self.trigger_compile_now(ctx);
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

    fn navigate_editor_to_source(
        &mut self,
        ctx: &Context,
        file: &std::path::Path,
        selection: CharSelection,
        scroll_to_char: usize,
    ) {
        let norm = crate::core::diff::normalize_rel_path(&file.to_string_lossy());
        let target_path = PathBuf::from(norm);
        if (target_path != self.project.active_file
            || !self.project.open_tabs.iter().any(|p| p == &target_path))
            && self.project.open_file(&target_path).is_err()
        {
            return;
        }
        let max_chars = self
            .project
            .active_buffer()
            .and_then(|b| b.text.as_deref())
            .map(|t| t.chars().count());
        let clamped_sel = max_chars.map_or(selection, |n| CharSelection {
            primary_char: selection.primary_char.min(n),
            secondary_char: selection.secondary_char.min(n),
        });
        let clamped_scroll = max_chars.map_or(scroll_to_char, |n| scroll_to_char.min(n));
        self.inspect_asset_mode = false;
        self.pending_cursor_restore = Some(clamped_sel);
        self.pending_scroll_to_char = Some(clamped_scroll);
        self.last_editor_selection = Some(clamped_sel);
        ctx.request_repaint();
    }

    fn render_preview_pane(&mut self, ctx: &Context, ui: &mut Ui) {
        let output = crate::ui::components::render_preview_pane(
            ui,
            crate::ui::components::PreviewPaneParams {
                dark_mode: self.settings.dark_mode,
                zoom: &mut self.zoom,
                sync_selection: &mut self.settings.preview_sync_selection,
                pan_mode: &mut self.settings.preview_pan_mode,
                page_textures: &self.page_textures,
                diagnostics: &self.diagnostics,
                selection: &mut self.preview_selection,
            },
        );
        if let Some(file_to_open) = output.open_diagnostic_file {
            self.navigate_editor_to_source(ctx, &file_to_open, CharSelection::caret(0), 0);
        }
        if let Some(jump) = output.source_jump {
            self.navigate_editor_to_source(ctx, &jump.file, jump.selection, jump.scroll_to_char);
        }
        if let Some(copied) = output.copied_text {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(copied.clone());
            }
            self.clipboard_fallback = copied;
        }
    }
}

impl eframe::App for AzoTypeApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.poll_channels(ctx);
        if self.settings.theme_preference == ThemePreference::SystemDefault {
            let resolved =
                crate::platform::resolve_effective_dark_mode(ctx, self.settings.theme_preference);
            if resolved != self.settings.dark_mode {
                self.settings.dark_mode = resolved;
                apply_theme(ctx, resolved);
            }
        }
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
                crate::ui::components::render_vertical_split_divider(
                    ctx,
                    ui,
                    &palette,
                    total_rect,
                    avail_w,
                    div_x,
                    &mut self.editor_split_ratio,
                );
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

        crate::services::handle_screenshot_lifecycle(
            ctx,
            self.screenshot_path.as_deref(),
            &mut self.frame_count,
            &mut self.screenshot_requested,
        );
        crate::services::flush_if_changed(
            frame,
            &mut self.last_persisted_state,
            &self.language,
            &self.settings,
            self.zoom,
        );
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        crate::services::save_persisted_parts(storage, &self.language, &self.settings, self.zoom);
    }
}
