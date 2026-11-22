//! Top-level application state orchestrator (`AzoTypeApp`), event loop, and channel dispatcher.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::{Align, Context, CornerRadius, Frame, Layout, Margin, Ui};

use crate::collab::awareness::PeerPresence;
use crate::collab::doc::CharSelection;
use crate::collab::session::{CollabSessionHandle, UiToSessionCmd};
use crate::collab::ui::{render_collaboration_window, CollabUiState};
use crate::compiler::{
    upload_page_textures, CompileSnapshot, CompiledOutlineItem, DiagnosticItem, PdfExportConfig,
    RenderedPageData, RenderedPageTexture,
};
use crate::core::recent_projects::RecentProjects;
use crate::core::spellcheck::UserDictionaryEntry;
use crate::engine::EngineHost;
use crate::highlight::ThemePalette;
use crate::language_check::LanguageCheckHost;
use crate::project::{DiskProject, FileKind};
use crate::ui::components::{
    compute_horizontal_split, compute_vertical_split, diagnostic_char_offset, render_editor_pane,
    render_horizontal_split_divider, render_preview_pane, render_sidebar, render_status_bar,
    render_vertical_split_divider, BottomPanelState, EditorIntellisenseState, EditorPaneParams,
    PreviewPaneParams, PreviewSelectionState, SidebarParams, StatusBarBufferInfo, StatusBarParams,
};
#[cfg(not(target_os = "macos"))]
use crate::ui::components::{render_desktop_menu_bar, MenuBarParams};
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
    DictionaryPicked(Result<UserDictionaryEntry, String>),
    StatusMessage(String),
    PdfExportFinished { status: String, console: String },
}

/// Backward-compatibility alias for [`AzoTypeApp`].
pub type TypstIdeApp = AzoTypeApp;

pub struct AzoTypeApp {
    pub language: String,
    pub engine_host: EngineHost,
    pub language_check: LanguageCheckHost,
    pub project: DiskProject,
    pub settings: IdeSettings,
    pub show_sidebar: bool,
    pub sidebar_split_ratio: f32,
    pub editor_split_ratio: f32,
    pub bottom_panel: BottomPanelState,
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
    pub editor_intellisense: EditorIntellisenseState,
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
    pub recent_projects: RecentProjects,
    pub last_persisted_state: crate::services::PersistedAppState,
}

impl AzoTypeApp {
    /// Constructs the application state from an `eframe::CreationContext`.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_folder: Option<PathBuf>,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        #[cfg(not(target_os = "macos"))]
        {
            Self::new_headless(&cc.egui_ctx, cc.storage, initial_folder, screenshot_path)
        }
        #[cfg(target_os = "macos")]
        {
            let mut app =
                Self::new_headless(&cc.egui_ctx, cc.storage, initial_folder, screenshot_path);
            app.macos_menu = MacOsMenuBar::init();
            if let Some(m) = app.macos_menu.as_mut() {
                m.update_recent_projects(app.recent_projects.entries());
            }
            app
        }
    }

    /// Constructs a headless-compatible `AzoTypeApp` from any `egui::Context` and optional storage.
    pub fn new_headless(
        ctx: &Context,
        storage: Option<&dyn eframe::Storage>,
        initial_folder: Option<PathBuf>,
        screenshot_path: Option<PathBuf>,
    ) -> Self {
        let resources = Arc::new(SharedWorldResources::new());
        let mut persisted =
            crate::services::load_persisted_state(storage, &rust_i18n::available_locales!());
        persisted.settings.dark_mode =
            crate::platform::resolve_effective_dark_mode(ctx, persisted.settings.theme_preference);
        let last_persisted_state = persisted.clone();
        rust_i18n::set_locale(&persisted.language);
        let project = crate::services::resolve_startup_project(
            initial_folder,
            persisted.settings.open_last_project_on_startup,
            &mut persisted.recent_projects,
            persisted.settings.show_hidden_files,
        );
        apply_theme(ctx, persisted.settings.dark_mode);
        let (engine_host, initial_snapshot) = EngineHost::init(
            resources,
            &project,
            &persisted.settings.typst_version,
            persisted.settings.render_dpi_scale,
        );
        let page_textures = upload_page_textures(ctx, &initial_snapshot.pages);
        let (io_tx, io_rx) = mpsc::channel();
        let (compile_tx, compile_rx) = mpsc::channel();
        let collab_session = CollabSessionHandle::spawn(Some(ctx.clone()));
        let language_check = LanguageCheckHost::new(Some(ctx.clone()));

        let status_notice = crate::services::project_actions::initial_project_status_notice(
            &project,
            engine_host.fallback_notice.clone(),
        );

        Self {
            language: persisted.language,
            engine_host,
            language_check,
            last_compiled_revision: initial_snapshot.revision,
            last_compiled_dpi: persisted.settings.render_dpi_scale,
            pending_compile: false,
            last_edit_time: 0.0,
            compile_in_progress: false,
            compile_duration_ms: initial_snapshot.duration_ms,
            page_data: initial_snapshot.pages,
            page_textures,
            compiled_outline: initial_snapshot.outline,
            diagnostics: initial_snapshot.diagnostics,
            project,
            settings: persisted.settings,
            show_sidebar: true,
            sidebar_split_ratio: 0.42,
            editor_split_ratio: 0.5,
            bottom_panel: BottomPanelState::default(),
            zoom: persisted.zoom,
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
            editor_intellisense: EditorIntellisenseState::default(),
            status_notice,
            io_tx,
            io_rx,
            compile_tx,
            compile_rx,
            #[cfg(target_os = "macos")]
            macos_menu: None,
            screenshot_path,
            frame_count: 0,
            screenshot_requested: false,
            recent_projects: persisted.recent_projects,
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

    fn finish_project_action(&mut self, ctx: &Context, ok: bool, notice: String) {
        self.status_notice = notice;
        if ok {
            self.trigger_compile_now(ctx);
        }
    }

    fn apply_optional_notice_and_compile(
        &mut self,
        ctx: &Context,
        notice: Option<String>,
        recompile: bool,
    ) {
        if let Some(msg) = notice {
            self.status_notice = msg;
        }
        if recompile {
            self.trigger_compile_now(ctx);
        }
    }

    fn maybe_auto_compile(&mut self, ctx: &Context) {
        if self.settings.auto_compile {
            self.trigger_compile_now(ctx);
        }
    }

    fn apply_folder_open(&mut self, ctx: &Context, path: &std::path::Path) {
        let (opened, notice) = crate::services::open_folder_and_record(
            path,
            self.settings.show_hidden_files,
            self.collab_ui.is_active,
            &self.collab_session,
            &mut self.project,
            &mut self.recent_projects,
        );
        self.status_notice = notice;
        if opened {
            self.preview_selection.clear();
            self.language_check.clear_results();
            self.trigger_compile_now(ctx);
        }
    }

    fn set_main_file_and_compile(&mut self, ctx: &Context, rel_path: PathBuf) {
        self.project.set_main_file(rel_path.clone());
        let p_str = rel_path.display().to_string();
        let notice = rust_i18n::t!("app.status_set_main", path = p_str).to_string();
        self.finish_project_action(ctx, true, notice);
    }

    /// Dispatches a top-level menu or keyboard-shortcut action.
    pub fn dispatch_menu_action(&mut self, ctx: &Context, action: MenuAction) {
        match action {
            MenuAction::NewFile => self.show_new_file_input = true,
            MenuAction::OpenFolder => self.open_folder_dialog(ctx),
            MenuAction::OpenRecentProject(idx) => {
                if let Some(path) = self.recent_projects.entries().get(idx).cloned() {
                    self.apply_folder_open(ctx, &path);
                }
            }
            MenuAction::ClearRecentProjects => self.recent_projects.clear(),
            MenuAction::RescanFolder => {
                let (ok, notice) = crate::services::rescan_project_folder(&mut self.project);
                self.finish_project_action(ctx, ok, notice);
            }
            MenuAction::SaveFile => {
                if let Some((ok, notice)) = crate::services::save_active_or_prompt(
                    ctx,
                    &mut self.project,
                    self.io_tx.clone(),
                    IoEvent::FileSavedAs,
                    IoEvent::StatusMessage,
                ) {
                    self.finish_project_action(ctx, ok, notice);
                }
            }
            MenuAction::SaveAll => {
                let (ok, notice) = crate::services::save_all_project_files(&mut self.project);
                self.finish_project_action(ctx, ok, notice);
            }
            MenuAction::ExportPdf => {
                self.show_export_pdf_dialog = true;
                self.export_pdf_error = None;
            }
            MenuAction::OpenSettings => self.show_settings_window = true,
            MenuAction::OpenCollab => self.collab_ui.show_window = true,
            MenuAction::Find => self.bottom_panel.open_search(),
            MenuAction::FindAndReplace => self.bottom_panel.open_replace(),
            act @ (MenuAction::ToggleLineComment
            | MenuAction::ToggleBlockComment
            | MenuAction::IndentLines
            | MenuAction::OutdentLines
            | MenuAction::DuplicateLine
            | MenuAction::MoveLineUp
            | MenuAction::MoveLineDown) => {
                self.pending_editor_context_action = Some(EditorContextAction::Menu(act));
            }
            MenuAction::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            MenuAction::ToggleBottomPanel => self.bottom_panel.toggle_visible(),
            MenuAction::ToggleWordWrap => self.settings.word_wrap = !self.settings.word_wrap,
            MenuAction::ToggleTheme => {
                self.settings.dark_mode = !self.settings.dark_mode;
                self.settings.theme_preference = if self.settings.dark_mode {
                    ThemePreference::Dark
                } else {
                    ThemePreference::Light
                };
                apply_theme(ctx, self.settings.dark_mode);
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
        let errs = self.diagnostics.iter().filter(|d| d.is_error).count();
        let warns = self.diagnostics.len().saturating_sub(errs);
        self.bottom_panel.push_console_log(format!(
            "[compile] {} ms | {errs} error(s), {warns} warning(s)",
            snapshot.duration_ms.round() as u64
        ));
    }

    fn poll_channels(&mut self, ctx: &Context) {
        #[cfg(target_os = "macos")]
        while let Some(action) = self.macos_menu.as_ref().and_then(|menu| menu.poll_action()) {
            self.dispatch_menu_action(ctx, action);
        }

        while let Ok(snapshot) = self.compile_rx.try_recv() {
            self.apply_compile_snapshot(ctx, snapshot);
        }

        let (dl_notice, dl_recompile) = self
            .engine_host
            .poll_downloads(&mut self.settings.typst_version);
        self.apply_optional_notice_and_compile(ctx, dl_notice, dl_recompile);
        if let Some(msg) =
            self.language_check
                .poll(ctx, &self.settings, &self.project, &self.language)
        {
            self.status_notice = msg;
        }

        while let Ok(event) = self.io_rx.try_recv() {
            match event {
                IoEvent::FolderPicked(path) => self.apply_folder_open(ctx, &path),
                IoEvent::FileSavedAs(path) => {
                    let notice =
                        crate::services::save_active_to_path_notice(&mut self.project, &path);
                    self.finish_project_action(ctx, true, notice);
                }
                IoEvent::DictionaryPicked(res) => {
                    self.status_notice =
                        crate::services::language_actions::apply_picked_dictionary_result(
                            res,
                            &mut self.settings,
                            &mut self.language_check,
                            &self.project,
                            &self.language,
                        );
                }
                IoEvent::StatusMessage(msg) => self.status_notice = msg,
                IoEvent::PdfExportFinished { status, console } => {
                    self.status_notice = status;
                    self.bottom_panel.push_console_log(console);
                }
            }
        }

        // Poll background P2P Collaboration Actor events
        let (need_compile, collab_notice) = crate::services::poll_collab_session_events(
            &self.collab_session,
            &mut self.collab_ui,
            &mut self.project,
            &mut self.pending_cursor_restore,
        );
        self.apply_optional_notice_and_compile(ctx, collab_notice, need_compile);
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        let outcome = crate::platform::poll_keyboard_shortcuts(ctx, self.collab_ui.is_active);
        crate::services::send_collab_undo_redo(
            &self.collab_session,
            &self.project.active_file,
            outcome.collab_undo,
            outcome.collab_redo,
        );
        if let Some(action) = outcome.menu_action {
            self.dispatch_menu_action(ctx, action);
        }
    }

    fn open_folder_dialog(&self, ctx: &Context) {
        crate::services::spawn_project_folder_picker(
            ctx,
            &self.project,
            self.io_tx.clone(),
            IoEvent::FolderPicked,
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
            |status, console| IoEvent::PdfExportFinished { status, console },
        );
        outcome
            .console_logs
            .into_iter()
            .for_each(|line| self.bottom_panel.push_console_log(line));
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
        self.language_check.on_text_edited(ctx, &self.settings);
    }

    /// Renders the desktop application menu bar on Linux and Windows.
    /// On macOS, the menu bar is part of the OS at the top of the screen via `muda` and is NOT rendered in-window.
    #[cfg(not(target_os = "macos"))]
    fn render_desktop_menu_bar(&mut self, ctx: &Context) {
        let output = render_desktop_menu_bar(
            ctx,
            MenuBarParams {
                dark_mode: self.settings.dark_mode,
                show_sidebar: self.show_sidebar,
                show_bottom_panel: self.bottom_panel.visible,
                word_wrap: self.settings.word_wrap,
                project_root: &self.project.root_dir,
                active_file: &self.project.active_file,
                recent_projects: self.recent_projects.entries(),
            },
        );

        if let Some(act) = output.editor_action {
            self.pending_editor_context_action = Some(act);
        }

        if let Some(snippet) = output.snippet_to_insert {
            let updated = crate::services::bottom_panel_actions::append_snippet_to_active_buffer(
                &mut self.project,
                snippet,
            );
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
            let (ok, notice) = crate::services::create_project_file(
                &mut self.project,
                self.collab_ui.is_active,
                &self.collab_session,
                &new_file_str,
            );
            self.finish_project_action(ctx, ok, notice);
        }

        if let Some((old_rel, new_str)) = crate::ui::dialogs::file_modals::render_rename_file_modal(
            ctx,
            &mut self.rename_target_file,
            &mut self.rename_file_input,
            self.settings.dark_mode,
        ) {
            let (ok, notice) = crate::services::rename_project_file(
                &mut self.project,
                self.collab_ui.is_active,
                &self.collab_session,
                &old_rel,
                &new_str,
            );
            self.finish_project_action(ctx, ok, notice);
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
            &self.language_check.dict_manager,
        );

        if let Some(ref l) = output.language_changed {
            self.set_language(ctx, l.clone());
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
        crate::services::language_actions::handle_settings_language_actions(
            ctx,
            &output,
            &mut self.settings,
            &mut self.language_check,
            &self.project,
            &self.language,
            &self.io_tx,
            IoEvent::DictionaryPicked,
            &mut self.status_notice,
        );
        let (eng_notice, eng_recompile) = self.engine_host.manager.handle_settings_actions(
            output.download_typst_version,
            output.remove_typst_version,
            output.engine_changed,
            &self.settings.typst_version,
            &mut self.engine_host.fallback_notice,
            &self.engine_host.tx,
            ctx,
        );
        self.apply_optional_notice_and_compile(
            ctx,
            eng_notice,
            output.dpi_changed || eng_recompile,
        );
    }

    fn render_bottom_status_bar(&mut self, ctx: &Context) {
        let active_buf_info = self.project.active_buffer().map(|b| StatusBarBufferInfo {
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

        let status_out = render_status_bar(
            ctx,
            StatusBarParams {
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
        let output = render_sidebar(
            ctx,
            SidebarParams {
                dark_mode: self.settings.dark_mode,
                show_sidebar: self.show_sidebar,
                sidebar_split_ratio: &mut self.sidebar_split_ratio,
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
            self.status_notice = crate::services::format_outline_nav_notice(&target_item);
        }
        if let Some(rel_path) = output.clicked_path {
            match crate::services::open_sidebar_file(&mut self.project, &rel_path) {
                Ok(is_asset) => {
                    self.inspect_asset_mode = is_asset;
                    self.maybe_auto_compile(ctx);
                }
                Err(msg) => self.status_notice = msg,
            }
        }
        if let Some(main_p) = output.new_main_path {
            self.set_main_file_and_compile(ctx, main_p);
        }
        if let Some(rel_to_rename) = output.rename_req {
            self.rename_file_input = rel_to_rename.to_string_lossy().to_string();
            self.rename_target_file = Some(rel_to_rename);
        }
        if let Some(rel_to_delete) = output.delete_req {
            let (ok, notice) = crate::services::delete_project_file(
                &mut self.project,
                self.collab_ui.is_active,
                &self.collab_session,
                &rel_to_delete,
            );
            self.finish_project_action(ctx, ok, notice);
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

        let output = render_editor_pane(
            ctx,
            ui,
            EditorPaneParams {
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
                language_diagnostics: if self.settings.spell_check_enabled
                    && self.language_check.checked_file == active_path
                {
                    &self.language_check.diagnostics
                } else {
                    &[]
                },
                compiler_diagnostics: &self.diagnostics,
                intellisense: &mut self.editor_intellisense,
            },
        );

        if let Some(tab) = output.select_tab {
            self.preview_selection.clear();
            let _ = self.project.open_file(&tab);
            self.maybe_auto_compile(ctx);
        }
        if let Some(tab) = output.close_tab {
            self.preview_selection.clear();
            self.project.close_tab(&tab);
            self.maybe_auto_compile(ctx);
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
                EditorContextAction::Menu(menu_act)
                    if !crate::services::bottom_panel_actions::is_inline_editor_menu_action(
                        menu_act,
                    ) =>
                {
                    self.dispatch_menu_action(ctx, menu_act);
                }
                EditorContextAction::SetActiveAsMain
                    if FileKind::from_path(&active_path) == FileKind::Typst =>
                {
                    self.set_main_file_and_compile(ctx, active_path.clone());
                }
                _ => {}
            }
        }

        crate::services::sync_collab_cursor(
            self.collab_ui.is_active,
            &self.collab_session,
            &mut self.last_sent_cursor,
            active_path,
            output.observed_cursor,
        );
    }

    fn navigate_editor_to_source(
        &mut self,
        ctx: &Context,
        file: &std::path::Path,
        selection: CharSelection,
        scroll_to_char: usize,
    ) {
        if let Some((clamped_sel, clamped_scroll)) =
            crate::services::project_actions::open_and_clamp_source(
                &mut self.project,
                file,
                selection,
                scroll_to_char,
            )
        {
            self.inspect_asset_mode = false;
            self.pending_cursor_restore = Some(clamped_sel);
            self.pending_scroll_to_char = Some(clamped_scroll);
            self.last_editor_selection = Some(clamped_sel);
            ctx.request_repaint();
        }
    }

    fn render_preview_pane(&mut self, ctx: &Context, ui: &mut Ui) {
        let output = render_preview_pane(
            ui,
            PreviewPaneParams {
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
    fn render_editor_preview_split(&mut self, ctx: &Context, ui: &mut Ui) {
        let total_rect = ui.available_rect_before_wrap();
        let split = compute_vertical_split(total_rect, 4.0, 220.0, 340.0, self.editor_split_ratio);

        let mut left_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(split.first_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        self.render_editor_pane(ctx, &mut left_ui);

        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(split.second_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        self.render_preview_pane(ctx, &mut right_ui);

        render_vertical_split_divider(
            ctx,
            ui,
            &ThemePalette::for_mode(self.settings.dark_mode),
            total_rect,
            &split,
            &mut self.editor_split_ratio,
        );
    }

    fn jump_to_diagnostic(&mut self, ctx: &Context, diag: DiagnosticItem) {
        let char_offset = diagnostic_char_offset(&self.project, &diag);
        self.navigate_editor_to_source(
            ctx,
            &diag.file,
            CharSelection::caret(char_offset),
            char_offset,
        );
    }

    /// Runs a full UI frame without requiring an `eframe::Frame`, enabling headless tests.
    pub fn render_ui(&mut self, ctx: &Context) {
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
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(palette.chrome_sidebar)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                if !self.bottom_panel.visible {
                    self.render_editor_preview_split(ctx, ui);
                } else {
                    let total_rect = ui.available_rect_before_wrap();
                    let content_split = compute_horizontal_split(
                        total_rect,
                        6.0,
                        160.0,
                        110.0,
                        self.bottom_panel.split_ratio,
                    );

                    let mut top_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(content_split.first_rect)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    self.render_editor_preview_split(ctx, &mut top_ui);

                    let mut bottom_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(content_split.second_rect)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    let (open_diag, edited_text) =
                        crate::services::bottom_panel_actions::render_and_handle_bottom_panel(
                            ctx,
                            &mut bottom_ui,
                            &mut self.bottom_panel,
                            &self.diagnostics,
                            &mut self.settings,
                            &mut self.language_check,
                            &mut self.project,
                            &self.language,
                            self.last_editor_selection,
                            &mut self.inspect_asset_mode,
                            &mut self.pending_cursor_restore,
                            &mut self.pending_scroll_to_char,
                            &mut self.show_settings_window,
                            &mut self.settings_tab,
                        );
                    if let Some(diag) = open_diag {
                        self.jump_to_diagnostic(ctx, diag);
                    }
                    if let Some(edited) = edited_text {
                        self.on_active_text_modified(ctx, Some(edited));
                    }

                    render_horizontal_split_divider(
                        ctx,
                        ui,
                        &palette,
                        "content_bottom_panel_split_divider",
                        "Drag to resize Workspace and Bottom Panel (double-click to reset)",
                        total_rect,
                        &content_split,
                        0.72,
                        &mut self.bottom_panel.split_ratio,
                    );
                }
            });

        // Native OS Sub-Windows (Settings, Export PDF, New/Rename File, P2P Collaboration)
        self.render_settings_subwindow(ctx);
        self.render_export_pdf_dialog(ctx);
        self.render_file_dialogs_subwindows(ctx);

        let collab_actions = render_collaboration_window(
            ctx,
            self.settings.dark_mode,
            &self.project.root_dir,
            &mut self.settings.user_name,
            &mut self.settings.user_color,
            &mut self.collab_ui,
        );
        crate::services::dispatch_collab_window_actions(
            ctx,
            &self.project.root_dir,
            &self.collab_session,
            collab_actions,
        );
        crate::services::handle_screenshot_lifecycle(
            ctx,
            self.screenshot_path.as_deref(),
            &mut self.frame_count,
            &mut self.screenshot_requested,
        );
    }
}

impl eframe::App for AzoTypeApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.render_ui(ctx);
        #[cfg(target_os = "macos")]
        if self.recent_projects != self.last_persisted_state.recent_projects {
            if let Some(m) = self.macos_menu.as_mut() {
                m.update_recent_projects(self.recent_projects.entries());
            }
        }
        crate::services::flush_if_changed(
            frame,
            &mut self.last_persisted_state,
            &self.language,
            &self.settings,
            self.zoom,
            &self.recent_projects,
        );
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        crate::services::save_persisted_parts(
            storage,
            &self.language,
            &self.settings,
            self.zoom,
            &self.recent_projects,
        );
    }
}

#[cfg(test)]
#[path = "tests/app_tests.rs"]
mod tests;
