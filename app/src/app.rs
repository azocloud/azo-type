use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{
    Align, Color32, CollapsingHeader, Context, CornerRadius, FontFamily, FontId, Frame, Layout,
    Margin, Pos2, Rect, RichText, ScrollArea, Sense, SidePanel, Stroke, StrokeKind, TextEdit,
    TopBottomPanel, Ui, Vec2, Visuals,
};

use crate::compiler::{
    compile_workspace, export_pdf_bytes, upload_page_textures, CompileSnapshot, DiagnosticItem,
    RenderedPageData, RenderedPageTexture,
};
use crate::highlight::highlight_typst_code;
use crate::vfs::{FileKind, ProjectWorkspace};
use crate::world::SharedWorldResources;

pub enum IoEvent {
    FilesOpened(Vec<(String, Vec<u8>)>),
    #[cfg(not(target_arch = "wasm32"))]
    FolderLoaded(Result<ProjectWorkspace, String>),
    StatusMessage(String),
}

pub struct TypstIdeApp {
    pub resources: Arc<SharedWorldResources>,
    pub workspace: ProjectWorkspace,
    pub show_sidebar: bool,
    pub dark_mode: bool,
    pub zoom: f32,
    pub render_dpi_scale: f32,
    pub editor_font_size: f32,

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

    // New file creation modal/inline state
    pub show_new_file_input: bool,
    pub new_file_name: String,
    pub inspect_asset_mode: bool,

    // Status bar notification
    pub status_notice: String,

    // Async file dialog & background compilation channels
    pub io_tx: Sender<IoEvent>,
    pub io_rx: Receiver<IoEvent>,
    pub compile_tx: Sender<CompileSnapshot>,
    pub compile_rx: Receiver<CompileSnapshot>,

    // Screenshot automation
    pub screenshot_path: Option<PathBuf>,
    pub frame_count: u32,
    pub screenshot_requested: bool,
}

impl TypstIdeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot_path: Option<PathBuf>) -> Self {
        let resources = Arc::new(SharedWorldResources::new());
        let workspace = ProjectWorkspace::new_sample_project();

        let dark_mode = true;
        Self::apply_theme(&cc.egui_ctx, dark_mode);

        let render_dpi_scale = 2.0;
        let initial_snapshot = compile_workspace(
            Arc::clone(&resources),
            &workspace,
            render_dpi_scale,
        );
        let page_textures = upload_page_textures(&cc.egui_ctx, &initial_snapshot.pages);

        let (io_tx, io_rx) = mpsc::channel();
        let (compile_tx, compile_rx) = mpsc::channel();

        let font_families = resources.font_family_count();
        let font_faces = resources.font_face_count();

        Self {
            resources,
            last_compiled_revision: initial_snapshot.revision,
            last_compiled_dpi: render_dpi_scale,
            pending_compile: false,
            last_edit_time: 0.0,
            compile_in_progress: false,
            compile_duration_ms: initial_snapshot.duration_ms,
            page_data: initial_snapshot.pages,
            page_textures,
            diagnostics: initial_snapshot.diagnostics,
            workspace,
            show_sidebar: true,
            dark_mode,
            zoom: 1.0,
            render_dpi_scale,
            editor_font_size: 13.5,
            show_new_file_input: false,
            new_file_name: "chapters/appendix.typ".to_string(),
            inspect_asset_mode: false,
            status_notice: format!(
                "Ready — Embedded {} font families ({} faces) in memory",
                font_families, font_faces
            ),
            io_tx,
            io_rx,
            compile_tx,
            compile_rx,
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

    fn trigger_compile_now(&mut self, ctx: &Context) {
        self.pending_compile = false;
        let resources = Arc::clone(&self.resources);
        let workspace_clone = self.workspace.clone();
        let dpi = self.render_dpi_scale;

        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.compile_in_progress {
                self.compile_in_progress = true;
                let tx = self.compile_tx.clone();
                let ctx_clone = ctx.clone();
                std::thread::spawn(move || {
                    let snapshot = compile_workspace(resources, &workspace_clone, dpi);
                    let _ = tx.send(snapshot);
                    ctx_clone.request_repaint();
                });
            } else {
                // Re-queue once in-flight pass completes
                self.pending_compile = true;
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let snapshot = compile_workspace(resources, &workspace_clone, dpi);
            self.apply_compile_snapshot(ctx, snapshot);
        }
    }

    fn apply_compile_snapshot(&mut self, ctx: &Context, snapshot: CompileSnapshot) {
        self.compile_in_progress = false;
        self.last_compiled_revision = snapshot.revision;
        self.last_compiled_dpi = snapshot.pixel_per_pt;
        self.compile_duration_ms = snapshot.duration_ms;
        self.diagnostics = snapshot.diagnostics;

        // Keep the previous valid rendered pages on screen if a transient syntax error occurs
        // while the user is mid-keystroke, or update if compilation succeeded.
        if !snapshot.pages.is_empty() {
            self.page_textures = upload_page_textures(ctx, &snapshot.pages);
            self.page_data = snapshot.pages;
        }
    }

    fn poll_channels(&mut self, ctx: &Context) {
        while let Ok(snapshot) = self.compile_rx.try_recv() {
            self.apply_compile_snapshot(ctx, snapshot);
        }

        while let Ok(event) = self.io_rx.try_recv() {
            match event {
                IoEvent::FilesOpened(items) => {
                    let count = items.len();
                    for (name, bytes) in items {
                        self.workspace.upsert_binary_file(&name, bytes);
                    }
                    self.status_notice = format!("Loaded {} file(s) into workspace", count);
                    self.trigger_compile_now(ctx);
                }
                #[cfg(not(target_arch = "wasm32"))]
                IoEvent::FolderLoaded(result) => match result {
                    Ok(ws) => {
                        let count = ws.files.len();
                        let name = ws.project_name.clone();
                        self.workspace = ws;
                        self.status_notice =
                            format!("Opened folder '{}' ({} files)", name, count);
                        self.trigger_compile_now(ctx);
                    }
                    Err(err) => {
                        self.status_notice = format!("Folder load error: {}", err);
                    }
                },
                IoEvent::StatusMessage(msg) => {
                    self.status_notice = msg;
                }
            }
        }
    }

    fn open_files_dialog(&self, ctx: &Context) {
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();
        let task = async move {
            if let Some(handles) = rfd::AsyncFileDialog::new()
                .set_title("Import Files into Typst Workspace")
                .add_filter(
                    "Typst & Assets",
                    &["typ", "bib", "yml", "svg", "png", "jpg", "jpeg", "txt", "csv"],
                )
                .pick_files()
                .await
            {
                let mut loaded = Vec::new();
                for handle in handles {
                    let name = handle.file_name();
                    let bytes = handle.read().await;
                    loaded.push((name, bytes));
                }
                if !loaded.is_empty() {
                    let _ = tx.send(IoEvent::FilesOpened(loaded));
                    ctx_clone.request_repaint();
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            pollster::block_on_or_spin(task);
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(task);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_folder_dialog(&self, ctx: &Context) {
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();
        let task = async move {
            if let Some(folder) = rfd::AsyncFileDialog::new()
                .set_title("Open Typst Project Folder")
                .pick_folder()
                .await
            {
                let path = folder.path().to_path_buf();
                let res = ProjectWorkspace::load_from_native_dir(&path);
                let _ = tx.send(IoEvent::FolderLoaded(res));
                ctx_clone.request_repaint();
            }
        };

        std::thread::spawn(move || {
            pollster::block_on_or_spin(task);
        });
    }

    fn save_active_or_project(&mut self, ctx: &Context) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(root) = self.workspace.root_dir.clone() {
            match self.workspace.save_all_to_native_dir(&root) {
                Ok(n) => {
                    self.status_notice =
                        format!("Saved {} workspace files to {}", n, root.display());
                    return;
                }
                Err(e) => {
                    self.status_notice = format!("Failed to save project: {}", e);
                    return;
                }
            }
        }

        // Otherwise save the active file via rfd::AsyncFileDialog
        let Some(active) = self.workspace.get_file_mut(&self.workspace.active_file.clone()) else {
            return;
        };
        active.sync_bytes_from_text();
        active.dirty = false;
        let suggested = active
            .path
            .rsplit('/')
            .next()
            .unwrap_or("main.typ")
            .to_string();
        let bytes = active.bytes.clone();
        let tx = self.io_tx.clone();
        let ctx_clone = ctx.clone();

        let task = async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Save File")
                .set_file_name(&suggested)
                .save_file()
                .await
            {
                match handle.write(&bytes).await {
                    Ok(()) => {
                        let _ = tx.send(IoEvent::StatusMessage(format!(
                            "Saved '{}'",
                            handle.file_name()
                        )));
                    }
                    Err(e) => {
                        let _ = tx.send(IoEvent::StatusMessage(format!("Save error: {}", e)));
                    }
                }
                ctx_clone.request_repaint();
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            pollster::block_on_or_spin(task);
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(task);
    }

    fn export_pdf_action(&mut self, ctx: &Context) {
        let resources = Arc::clone(&self.resources);
        let workspace_snapshot = self.workspace.clone();
        let default_pdf_name = format!(
            "{}.pdf",
            self.workspace
                .main_file
                .trim_end_matches(".typ")
                .replace('/', "_")
        );

        match export_pdf_bytes(resources, &workspace_snapshot) {
            Ok(pdf_bytes) => {
                let size_kb = pdf_bytes.len() as f64 / 1024.0;
                self.status_notice = format!(
                    "Compiled PDF ({:.1} KB) — Select destination in dialog...",
                    size_kb
                );
                let tx = self.io_tx.clone();
                let ctx_clone = ctx.clone();

                let task = async move {
                    if let Some(handle) = rfd::AsyncFileDialog::new()
                        .set_title("Export Compiled PDF")
                        .set_file_name(&default_pdf_name)
                        .add_filter("PDF Document", &["pdf"])
                        .save_file()
                        .await
                    {
                        match handle.write(&pdf_bytes).await {
                            Ok(()) => {
                                let _ = tx.send(IoEvent::StatusMessage(format!(
                                    "Exported PDF to '{}' ({:.1} KB)",
                                    handle.file_name(),
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

                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    pollster::block_on_or_spin(task);
                });

                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(task);
            }
            Err(diags) => {
                self.diagnostics = diags;
                self.status_notice =
                    "Cannot export PDF: fix compilation errors shown in the banner.".to_string();
            }
        }
    }

    fn render_top_menu_bar(&mut self, ctx: &Context) {
        let bar_bg = if self.dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let border_col = if self.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        TopBottomPanel::top("top_menu_bar")
            .frame(
                Frame::new()
                    .fill(bar_bg)
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    // Sidebar Toggle Button
                    let sidebar_label = if self.show_sidebar {
                        "Hide Files"
                    } else {
                        "Show Files"
                    };
                    if ui
                        .selectable_label(
                            self.show_sidebar,
                            RichText::new(sidebar_label).strong(),
                        )
                        .on_hover_text("Toggle Left File Overview Tree")
                        .clicked()
                    {
                        self.show_sidebar = !self.show_sidebar;
                    }

                    ui.separator();

                    // Brand Title
                    ui.label(
                        RichText::new("TYPST STUDIO")
                            .strong()
                            .size(14.0)
                            .color(if self.dark_mode {
                                Color32::from_rgb(56, 189, 248)
                            } else {
                                Color32::from_rgb(2, 132, 199)
                            }),
                    );

                    ui.separator();

                    // Project Actions: New File, Open Files, Load Folder, Save
                    if ui
                        .button("+ New File")
                        .on_hover_text("Create a new .typ or asset file in the VFS workspace")
                        .clicked()
                    {
                        self.show_new_file_input = !self.show_new_file_input;
                    }

                    if ui
                        .button("Open Files...")
                        .on_hover_text("Import files from disk/browser into the VFS")
                        .clicked()
                    {
                        self.open_files_dialog(ctx);
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    if ui
                        .button("Load Folder...")
                        .on_hover_text("Load an entire project directory from local disk")
                        .clicked()
                    {
                        self.open_folder_dialog(ctx);
                    }

                    if ui
                        .button("Save")
                        .on_hover_text("Save active file or sync workspace to disk")
                        .clicked()
                    {
                        self.save_active_or_project(ctx);
                    }

                    ui.separator();

                    // Zoom controls in top bar
                    ui.label(RichText::new("Zoom:").small());
                    if ui.small_button("-").clicked() {
                        self.zoom = (self.zoom - 0.15).clamp(0.4, 2.5);
                    }
                    if ui
                        .small_button(format!("{:.0}%", self.zoom * 100.0))
                        .on_hover_text("Click to reset zoom to 100%")
                        .clicked()
                    {
                        self.zoom = 1.0;
                    }
                    if ui.small_button("+").clicked() {
                        self.zoom = (self.zoom + 0.15).clamp(0.4, 2.5);
                    }

                    // Right-aligned actions: Theme Switcher + Export PDF
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let export_btn = egui::Button::new(
                            RichText::new("Export PDF")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(37, 99, 235))
                        .corner_radius(CornerRadius::same(5));

                        if ui
                            .add(export_btn)
                            .on_hover_text("Compile document with typst-pdf and save PDF")
                            .clicked()
                        {
                            self.export_pdf_action(ctx);
                        }

                        ui.add_space(4.0);

                        let theme_label = if self.dark_mode {
                            "Light Theme"
                        } else {
                            "Dark Theme"
                        };
                        if ui
                            .button(theme_label)
                            .on_hover_text("Switch between Light and Dark editor themes")
                            .clicked()
                        {
                            self.dark_mode = !self.dark_mode;
                            Self::apply_theme(ctx, self.dark_mode);
                        }

                        if ui
                            .button("Compile")
                            .on_hover_text("Force immediate Typst compilation")
                            .clicked()
                        {
                            self.trigger_compile_now(ctx);
                        }
                    });
                });

                // Optional inline New File bar
                if self.show_new_file_input {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("New virtual path:").strong());
                        let resp = ui.add(
                            TextEdit::singleline(&mut self.new_file_name)
                                .desired_width(240.0)
                                .hint_text("e.g. chapters/results.typ"),
                        );
                        let enter_pressed =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button("Create").clicked() || enter_pressed {
                            let path = self.new_file_name.trim().to_string();
                            if !path.is_empty() {
                                let starter = if path.ends_with(".typ") {
                                    format!("== {}\n\nWrite content for `{}` here.\n", path, path)
                                } else {
                                    String::new()
                                };
                                let key = self.workspace.upsert_text_file(&path, starter);
                                self.status_notice = format!("Created '{}' in VFS", key);
                                self.show_new_file_input = false;
                                self.trigger_compile_now(ctx);
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_new_file_input = false;
                        }
                    });
                }
            });
    }

    fn render_bottom_status_bar(&mut self, ctx: &Context) {
        let bar_bg = if self.dark_mode {
            Color32::from_rgb(11, 15, 25)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let border_col = if self.dark_mode {
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
                    // Status pill
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
                        RichText::new(format!("Entrypoint: {}", self.workspace.main_file))
                            .small(),
                    );

                    ui.separator();
                    ui.label(
                        RichText::new(format!("Pages: {}", self.page_textures.len())).small(),
                    );

                    ui.separator();
                    ui.label(RichText::new(&self.status_notice).small().weak());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(active) = self.workspace.get_file(&self.workspace.active_file) {
                            ui.label(
                                RichText::new(format!(
                                    "{}  |  {} lines  |  {} bytes",
                                    active.path,
                                    active.line_count(),
                                    active.size_bytes()
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

        let panel_bg = if self.dark_mode {
            Color32::from_rgb(15, 23, 42)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        let border_col = if self.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        SidePanel::left("left_file_tree_panel")
            .resizable(true)
            .default_width(245.0)
            .width_range(180.0..=380.0)
            .frame(
                Frame::new()
                    .fill(panel_bg)
                    .inner_margin(Margin::symmetric(10, 10))
                    .stroke(Stroke::new(1.0_f32, border_col)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PROJECT EXPLORER")
                            .size(11.5)
                            .strong()
                            .color(if self.dark_mode {
                                Color32::from_rgb(148, 163, 184)
                            } else {
                                Color32::from_rgb(71, 85, 105)
                            }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button("+ File")
                            .on_hover_text("Create new file in project")
                            .clicked()
                        {
                            self.show_new_file_input = true;
                        }
                    });
                });

                ui.add_space(6.0);

                // Project Root Header
                let mut clicked_path: Option<String> = None;
                let mut new_main_path: Option<String> = None;

                let active_path = self.workspace.active_file.clone();
                let main_path = self.workspace.main_file.clone();
                let project_title = format!("Workspace: {}", self.workspace.project_name);

                ScrollArea::vertical()
                    .id_salt("file_tree_scroll")
                    .show(ui, |ui| {
                        CollapsingHeader::new(RichText::new(project_title).strong())
                            .default_open(true)
                            .show(ui, |ui| {
                                let groups = self.workspace.grouped_by_folder();

                                // 1. Top-level root files
                                if let Some(root_files) = groups.get("") {
                                    for file in root_files {
                                        Self::render_tree_file_row(
                                            ui,
                                            file.path.as_str(),
                                            file.path.as_str(),
                                            file.kind,
                                            file.dirty,
                                            file.path == active_path,
                                            file.path == main_path,
                                            &mut clicked_path,
                                            &mut new_main_path,
                                        );
                                    }
                                }

                                // 2. Subfolders
                                for (folder, folder_files) in &groups {
                                    if folder.is_empty() {
                                        continue;
                                    }
                                    let folder_header = format!("{}/", folder);
                                    CollapsingHeader::new(RichText::new(folder_header).strong())
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for file in folder_files {
                                                let short_name = file
                                                    .path
                                                    .rsplit('/')
                                                    .next()
                                                    .unwrap_or(file.path.as_str());
                                                Self::render_tree_file_row(
                                                    ui,
                                                    &file.path,
                                                    short_name,
                                                    file.kind,
                                                    file.dirty,
                                                    file.path == active_path,
                                                    file.path == main_path,
                                                    &mut clicked_path,
                                                    &mut new_main_path,
                                                );
                                            }
                                        });
                                }
                            });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // Active File / Asset Summary Box inside the sidebar
                        if let Some(active) = self.workspace.get_file(&self.workspace.active_file) {
                            ui.label(
                                RichText::new("ACTIVE ITEM METADATA")
                                    .size(10.5)
                                    .strong()
                                    .color(Color32::from_rgb(100, 116, 139)),
                            );
                            ui.add_space(4.0);
                            Frame::new()
                                .fill(if self.dark_mode {
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
                                            active.path
                                        ))
                                        .strong()
                                        .size(12.0),
                                    );
                                    ui.label(
                                        RichText::new(active.kind.label()).small().weak(),
                                    );
                                    ui.label(
                                        RichText::new(format!("Size: {} bytes", active.size_bytes()))
                                            .small(),
                                    );
                                    if active.kind == FileKind::Typst {
                                        let is_main = active.path == self.workspace.main_file;
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
                                            new_main_path = Some(active.path.clone());
                                        }
                                    } else if active.kind == FileKind::Svg
                                        || active.kind == FileKind::Image
                                    {
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "#image(\"{}\")",
                                                active.path
                                            ))
                                            .monospace()
                                            .small(),
                                        );
                                    }
                                });
                        }
                    });

                if let Some(path) = clicked_path {
                    self.workspace.select_file(&path);
                    if let Some(f) = self.workspace.get_file(&path) {
                        self.inspect_asset_mode =
                            matches!(f.kind, FileKind::Image | FileKind::Binary);
                    }
                }
                if let Some(main_p) = new_main_path {
                    self.workspace.set_main_file(&main_p);
                    self.status_notice = format!("Set '{}' as main Typst file", main_p);
                    self.trigger_compile_now(ctx);
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tree_file_row(
        ui: &mut Ui,
        full_path: &str,
        display_name: &str,
        kind: FileKind,
        dirty: bool,
        is_active: bool,
        is_main: bool,
        clicked_path: &mut Option<String>,
        new_main_path: &mut Option<String>,
    ) {
        ui.horizontal(|ui| {
            let dirty_mark = if dirty { " •" } else { "" };
            let main_badge = if is_main { " ★" } else { "" };
            let label_text = format!(
                "{} {}{}{}",
                kind.icon(),
                display_name,
                main_badge,
                dirty_mark
            );
            let mut rich = RichText::new(label_text).size(12.5);
            if is_active {
                rich = rich.strong();
            }
            let resp = ui.selectable_label(is_active, rich);
            if resp.clicked() {
                *clicked_path = Some(full_path.to_string());
            }
            resp.context_menu(|ui| {
                ui.label(RichText::new(full_path).monospace().small());
                ui.separator();
                if kind == FileKind::Typst && !is_main && ui.button("Set as Main File").clicked() {
                    *new_main_path = Some(full_path.to_string());
                    ui.close_menu();
                }
            });
        });
    }

    fn render_editor_pane(&mut self, ctx: &Context, ui: &mut Ui) {
        let border_col = if self.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        // 1. Open File Tabs Bar
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let tabs = self.workspace.open_tabs.clone();
            let active_path = self.workspace.active_file.clone();
            let mut select_tab = None;
            let mut close_tab = None;

            for tab_path in &tabs {
                let is_active = *tab_path == active_path;
                let short_name = tab_path.rsplit('/').next().unwrap_or(tab_path);
                let icon = self
                    .workspace
                    .get_file(tab_path)
                    .map(|f| f.kind.icon())
                    .unwrap_or("📄");
                let tab_label = format!("{} {}", icon, short_name);

                if ui
                    .selectable_label(is_active, RichText::new(tab_label).size(12.0))
                    .on_hover_text(tab_path)
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
                self.workspace.select_file(&p);
            }
            if let Some(p) = close_tab {
                self.workspace.close_tab(&p);
            }
        });

        ui.separator();

        // 2. Quick Formatting & Snippet Toolbar
        let active_path = self.workspace.active_file.clone();
        let active_kind = self
            .workspace
            .get_file(&active_path)
            .map(|f| f.kind)
            .unwrap_or(FileKind::Typst);

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
                if let Some(file) = self.workspace.get_file_mut(&active_path) {
                    if let Some(ref mut text) = file.text {
                        text.push_str(snippet);
                        self.workspace.mark_modified(&active_path);
                        self.pending_compile = true;
                        self.last_edit_time = ctx.input(|i| i.time);
                    }
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("A+").on_hover_text("Increase editor font size").clicked() {
                    self.editor_font_size = (self.editor_font_size + 1.0).clamp(10.0, 22.0);
                }
                if ui.small_button("A-").on_hover_text("Decrease editor font size").clicked() {
                    self.editor_font_size = (self.editor_font_size - 1.0).clamp(10.0, 22.0);
                }
                ui.label(
                    RichText::new(format!("PATH: {}", active_path))
                        .size(11.0)
                        .monospace()
                        .weak(),
                );
            });
        });

        ui.separator();

        // 3. Editor Area or Asset Inspector
        let Some(file) = self.workspace.get_file_mut(&active_path) else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a file from the Project Explorer.");
            });
            return;
        };

        if file.text.is_none() || self.inspect_asset_mode {
            // Asset metadata / preview view
            Frame::new()
                .fill(if self.dark_mode {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::from_rgb(248, 250, 252)
                })
                .inner_margin(Margin::same(16))
                .corner_radius(CornerRadius::same(6))
                .stroke(Stroke::new(1.0_f32, border_col))
                .show(ui, |ui| {
                    ui.heading(format!("{} {}", file.kind.icon(), file.path));
                    ui.add_space(6.0);
                    ui.label(format!("Asset Type: {}", file.kind.label()));
                    ui.label(format!("Byte Length: {} bytes", file.size_bytes()));
                    ui.add_space(8.0);
                    ui.label("Embed in any Typst file using:");
                    ui.code(format!("#image(\"{}\", width: 80%)", file.path));
                    if file.text.is_some() {
                        ui.add_space(8.0);
                        if ui.button("Switch to Source Code Editor").clicked() {
                            self.inspect_asset_mode = false;
                        }
                    }
                });
            return;
        }

        let dark_mode = self.dark_mode;
        let font_size = self.editor_font_size;
        let is_typst = file.kind == FileKind::Typst;
        let mut text_changed = false;

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

                            // Line numbers gutter
                            Frame::new()
                                .fill(gutter_bg)
                                .inner_margin(Margin::symmetric(8, 8))
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(line_numbers_str.trim_end())
                                                .font(FontId::new(font_size, FontFamily::Monospace))
                                                .color(gutter_fg),
                                        )
                                        .selectable(false),
                                    );
                                });

                            // Code editor with custom Typst syntax highlighting
                            let mut layouter =
                                |ui: &Ui, string: &str, wrap_width: f32| {
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
                                    let resp = ui.add_sized(
                                        ui.available_size(),
                                        TextEdit::multiline(text_ref)
                                            .id_salt("typst_code_editor")
                                            .code_editor()
                                            .desired_width(f32::INFINITY)
                                            .desired_rows(34)
                                            .frame(false)
                                            .font(FontId::new(font_size, FontFamily::Monospace))
                                            .layouter(&mut layouter),
                                    );
                                    if resp.changed() {
                                        text_changed = true;
                                    }
                                });
                        });
                    });
            });

        if text_changed {
            self.workspace.mark_modified(&active_path);
            self.pending_compile = true;
            self.last_edit_time = ctx.input(|i| i.time);
        }
    }

    fn render_preview_pane(&mut self, _ctx: &Context, ui: &mut Ui) {
        let canvas_bg = if self.dark_mode {
            Color32::from_rgb(30, 41, 59)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        let border_col = if self.dark_mode {
            Color32::from_rgb(51, 65, 85)
        } else {
            Color32::from_rgb(203, 213, 225)
        };

        // Preview Top Header Bar
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("LIVE DOCUMENT PREVIEW")
                    .size(12.0)
                    .strong()
                    .color(if self.dark_mode {
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

        // Reserve space at the bottom if there are errors or warnings to show the diagnostic banner
        let has_diagnostics = !self.diagnostics.is_empty();
        let available_height = ui.available_height();
        let banner_height = if has_diagnostics {
            (available_height * 0.28).clamp(110.0, 195.0)
        } else {
            0.0
        };
        let preview_height = (available_height - banner_height - 4.0).max(120.0);

        // Scrollable & Zoomable Page Canvas
        ui.allocate_ui(Vec2::new(ui.available_width(), preview_height), |ui| {
            Frame::new()
                .fill(canvas_bg)
                .stroke(Stroke::new(1.0_f32, border_col))
                .corner_radius(CornerRadius::same(4))
                .show(ui, |ui| {
                    // Handle Ctrl + Scroll wheel zoom inside preview
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
                                        // Fit naturally into the preview pane width at zoom = 1.0
                                        let base_scale =
                                            (avail_w / page.width_pt).clamp(0.45, 1.35);
                                        let display_w = page.width_pt * base_scale * self.zoom;
                                        let display_h = page.height_pt * base_scale * self.zoom;
                                        let page_size = Vec2::new(display_w, display_h);

                                        let (rect, _response) =
                                            ui.allocate_exact_size(page_size, Sense::hover());

                                        // Subtle multi-layer drop shadow
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

                                        // White paper backing + rendered Typst page texture
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

        // Bottom Error & Diagnostic Banner if compilation fails or has warnings
        if has_diagnostics {
            ui.add_space(4.0);
            let error_count = self.diagnostics.iter().filter(|d| d.is_error).count();
            let banner_bg = if error_count > 0 {
                if self.dark_mode {
                    Color32::from_rgb(45, 18, 24)
                } else {
                    Color32::from_rgb(254, 242, 242)
                }
            } else if self.dark_mode {
                Color32::from_rgb(45, 34, 15)
            } else {
                Color32::from_rgb(255, 251, 235)
            };

            let banner_border = if error_count > 0 {
                Color32::from_rgb(239, 68, 68)
            } else {
                Color32::from_rgb(245, 158, 11)
            };

            let mut file_to_open: Option<String> = None;

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
                                        (Some(l), Some(c)) => format!("{}:{}:{}", diag.file, l, c),
                                        (Some(l), None) => format!("{}:{}", diag.file, l),
                                        _ => diag.file.clone(),
                                    };
                                    if ui
                                        .small_button(RichText::new(&loc).monospace().color(badge_color))
                                        .on_hover_text("Click to open file in editor")
                                        .clicked()
                                    {
                                        file_to_open = Some(diag.file.clone());
                                    }
                                    ui.label(RichText::new(&diag.message).strong().size(12.0));
                                });
                                for hint in &diag.hints {
                                    ui.label(
                                        RichText::new(format!("  💡 Hint: {}", hint))
                                            .italics()
                                            .size(11.5),
                                    );
                                }
                                ui.add_space(3.0);
                            }
                        });
                });

            if let Some(target_file) = file_to_open {
                self.workspace.select_file(&target_file);
            }
        }
    }

    fn handle_screenshot_lifecycle(&mut self, ctx: &Context) {
        let Some(ref target_path) = self.screenshot_path else {
            return;
        };

        // Keep requesting repaint so the window draws continuously even when unfocused
        ctx.request_repaint();
        self.frame_count = self.frame_count.saturating_add(1);

        // Check if a screenshot event arrived this frame
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

        // Debounced non-blocking compilation: wait 120ms after the last keystroke
        if self.pending_compile {
            let now = ctx.input(|i| i.time);
            if now - self.last_edit_time >= 0.12 {
                self.trigger_compile_now(ctx);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }

        // Top Menu Bar
        self.render_top_menu_bar(ctx);

        // Bottom Status Bar
        self.render_bottom_status_bar(ctx);

        // Collapsible Left File Overview Panel
        self.render_left_file_panel(ctx);

        // Main Split View (Code Editor Left + Live Preview Right)
        let central_bg = if self.dark_mode {
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
                let div_color = if self.dark_mode {
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

        self.handle_screenshot_lifecycle(ctx);
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
