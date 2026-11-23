//! Platform-specific native OS application menu integration.
//!
//! On macOS, installs a native top menu bar via `muda` with File, Edit, View, and Project submenus.
//! On Linux and Windows, native menus are replaced by the in-window immediate-mode menu bar.

/// User actions triggered from application menu items, keyboard shortcuts, or editor context actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    NewFile,
    OpenFolder,
    OpenRecentProject(usize),
    ClearRecentProjects,
    RescanFolder,
    SaveFile,
    SaveAll,
    ExportPdf,
    OpenSettings,
    OpenCollab,
    Find,
    FindAndReplace,
    ToggleLineComment,
    ToggleBlockComment,
    IndentLines,
    OutdentLines,
    DuplicateLine,
    MoveLineUp,
    MoveLineDown,
    ToggleSidebar,
    ToggleBottomPanel,
    ToggleWordWrap,
    ToggleTheme,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CompileNow,
    Quit,
}

#[cfg(target_os = "macos")]
pub use macos::MacOsMenuBar;

#[cfg(not(target_os = "macos"))]
pub struct MacOsMenuBar;

#[cfg(not(target_os = "macos"))]
impl MacOsMenuBar {
    pub fn init() -> Option<Self> {
        None
    }

    pub fn poll_action(&self) -> Option<MenuAction> {
        None
    }

    pub fn update_recent_projects(&mut self, _recent: &[std::path::PathBuf]) {}
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;

    use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    use super::MenuAction;
    use crate::core::recent_projects::RecentProjects;

    const MAX_RECENT_MENU_ITEMS: usize = 10;

    pub struct MacOsMenuBar {
        _menu: Menu,
        recent_submenu: Submenu,
        recent_items: Vec<(MenuId, usize)>,
        id_clear_recent: Option<MenuId>,
        id_new_file: MenuId,
        id_open_folder: MenuId,
        id_rescan: MenuId,
        id_save: MenuId,
        id_save_all: MenuId,
        id_export_pdf: MenuId,
        id_settings: MenuId,
        id_collab: MenuId,
        id_quit: MenuId,
        id_find: MenuId,
        id_find_replace: MenuId,
        id_toggle_line_comment: MenuId,
        id_toggle_block_comment: MenuId,
        id_indent_lines: MenuId,
        id_outdent_lines: MenuId,
        id_duplicate_line: MenuId,
        id_move_line_up: MenuId,
        id_move_line_down: MenuId,
        id_toggle_sidebar: MenuId,
        id_toggle_bottom_panel: MenuId,
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
            let recent_submenu = Submenu::new("Recently Opened", true);
            let item_rescan = MenuItem::new("Rescan Folder on Disk", true, None);
            let item_save = MenuItem::new("Save Active File", true, None);
            let item_save_all = MenuItem::new("Save All Project Files", true, None);
            let item_export_pdf = MenuItem::new("Export PDF...", true, None);
            let item_settings = MenuItem::new("Settings...", true, None);
            let item_quit = MenuItem::new("Quit Azo Type", true, None);

            let _ = file_menu.append_items(&[
                &item_new_file,
                &item_open_folder,
                &recent_submenu,
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

            let edit_menu = Submenu::new("Edit", true);
            let item_find = MenuItem::new("Find...", true, None);
            let item_find_replace = MenuItem::new("Find & Replace...", true, None);
            let item_toggle_line_comment = MenuItem::new("Toggle Line Comment", true, None);
            let item_toggle_block_comment = MenuItem::new("Toggle Block Comment", true, None);
            let item_indent_lines = MenuItem::new("Indent Lines", true, None);
            let item_outdent_lines = MenuItem::new("Outdent Lines", true, None);
            let item_duplicate_line = MenuItem::new("Duplicate Line", true, None);
            let item_move_line_up = MenuItem::new("Move Line Up", true, None);
            let item_move_line_down = MenuItem::new("Move Line Down", true, None);

            let _ = edit_menu.append_items(&[
                &item_find,
                &item_find_replace,
                &PredefinedMenuItem::separator(),
                &item_toggle_line_comment,
                &item_toggle_block_comment,
                &PredefinedMenuItem::separator(),
                &item_indent_lines,
                &item_outdent_lines,
                &item_duplicate_line,
                &item_move_line_up,
                &item_move_line_down,
            ]);

            let view_menu = Submenu::new("View", true);
            let item_toggle_sidebar = MenuItem::new("Toggle Project File Tree", true, None);
            let item_toggle_bottom_panel = MenuItem::new("Toggle Bottom Panel", true, None);
            let item_toggle_theme = MenuItem::new("Switch Theme (Dark / Light)", true, None);
            let item_zoom_in = MenuItem::new("Zoom In (+)", true, None);
            let item_zoom_out = MenuItem::new("Zoom Out (-)", true, None);
            let item_zoom_reset = MenuItem::new("Reset Zoom (100%)", true, None);
            let _ = view_menu.append_items(&[
                &item_toggle_sidebar,
                &item_toggle_bottom_panel,
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

            let _ = menu.append_items(&[&file_menu, &edit_menu, &view_menu, &project_menu]);

            let mut bar = Self {
                _menu: menu,
                recent_submenu,
                recent_items: Vec::new(),
                id_clear_recent: None,
                id_new_file: item_new_file.id().clone(),
                id_open_folder: item_open_folder.id().clone(),
                id_rescan: item_rescan.id().clone(),
                id_save: item_save.id().clone(),
                id_save_all: item_save_all.id().clone(),
                id_export_pdf: item_export_pdf.id().clone(),
                id_settings: item_settings.id().clone(),
                id_collab: item_collab.id().clone(),
                id_quit: item_quit.id().clone(),
                id_find: item_find.id().clone(),
                id_find_replace: item_find_replace.id().clone(),
                id_toggle_line_comment: item_toggle_line_comment.id().clone(),
                id_toggle_block_comment: item_toggle_block_comment.id().clone(),
                id_indent_lines: item_indent_lines.id().clone(),
                id_outdent_lines: item_outdent_lines.id().clone(),
                id_duplicate_line: item_duplicate_line.id().clone(),
                id_move_line_up: item_move_line_up.id().clone(),
                id_move_line_down: item_move_line_down.id().clone(),
                id_toggle_sidebar: item_toggle_sidebar.id().clone(),
                id_toggle_bottom_panel: item_toggle_bottom_panel.id().clone(),
                id_toggle_theme: item_toggle_theme.id().clone(),
                id_zoom_in: item_zoom_in.id().clone(),
                id_zoom_out: item_zoom_out.id().clone(),
                id_zoom_reset: item_zoom_reset.id().clone(),
                id_compile: item_compile.id().clone(),
            };
            bar.update_recent_projects(&[]);
            bar._menu.init_for_nsapp();

            Some(bar)
        }

        pub fn update_recent_projects(&mut self, recent: &[PathBuf]) {
            self.recent_items.clear();
            self.id_clear_recent = None;

            let existing_count = self.recent_submenu.items().len();
            for idx in (0..existing_count).rev() {
                self.recent_submenu.remove_at(idx);
            }

            if recent.is_empty() {
                let placeholder = MenuItem::new("(No Recent Projects)", false, None);
                let _ = self.recent_submenu.append(&placeholder);
                return;
            }

            for (idx, path) in recent.iter().take(MAX_RECENT_MENU_ITEMS).enumerate() {
                let item = MenuItem::new(RecentProjects::format_entry_label(path), true, None);
                self.recent_items.push((item.id().clone(), idx));
                let _ = self.recent_submenu.append(&item);
            }

            let clear_item = MenuItem::new("Clear Recently Opened", true, None);
            self.id_clear_recent = Some(clear_item.id().clone());
            let _ = self
                .recent_submenu
                .append_items(&[&PredefinedMenuItem::separator(), &clear_item]);
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
            } else if *id == self.id_find {
                Some(MenuAction::Find)
            } else if *id == self.id_find_replace {
                Some(MenuAction::FindAndReplace)
            } else if *id == self.id_toggle_line_comment {
                Some(MenuAction::ToggleLineComment)
            } else if *id == self.id_toggle_block_comment {
                Some(MenuAction::ToggleBlockComment)
            } else if *id == self.id_indent_lines {
                Some(MenuAction::IndentLines)
            } else if *id == self.id_outdent_lines {
                Some(MenuAction::OutdentLines)
            } else if *id == self.id_duplicate_line {
                Some(MenuAction::DuplicateLine)
            } else if *id == self.id_move_line_up {
                Some(MenuAction::MoveLineUp)
            } else if *id == self.id_move_line_down {
                Some(MenuAction::MoveLineDown)
            } else if *id == self.id_toggle_sidebar {
                Some(MenuAction::ToggleSidebar)
            } else if *id == self.id_toggle_bottom_panel {
                Some(MenuAction::ToggleBottomPanel)
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
            } else if self.id_clear_recent.as_ref() == Some(id) {
                Some(MenuAction::ClearRecentProjects)
            } else if let Some((_, idx)) =
                self.recent_items.iter().find(|(item_id, _)| item_id == id)
            {
                Some(MenuAction::OpenRecentProject(*idx))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/platform/menu_tests.rs"]
mod tests;
