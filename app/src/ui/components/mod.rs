//! UI reusable panels, controls, and components.

pub mod bottom_panel;
pub mod bottom_panel_autocorrect;
pub mod editor;
pub mod editor_actions;
pub mod editor_diagnostics;
pub mod menu_bar;
pub mod preview;
pub mod preview_selection;
pub mod sidebar;
pub mod split_divider;
pub mod status_bar;

pub use bottom_panel::{
    diagnostic_char_offset, render_bottom_panel, BottomPanelOutput, BottomPanelParams,
    BottomPanelState, BottomPanelTab,
};
pub use editor::{render_editor_pane, EditorPaneOutput, EditorPaneParams};
pub use menu_bar::{render_desktop_menu_bar, EditorContextAction, MenuBarOutput, MenuBarParams};
pub use preview::{
    render_preview_pane, PreviewPaneOutput, PreviewPaneParams, PreviewSelectionState,
    PreviewSourceJump,
};
pub use sidebar::{render_sidebar, SidebarOutput, SidebarParams};
pub use split_divider::{
    compute_horizontal_split, compute_vertical_split, render_horizontal_split_divider,
    render_vertical_split_divider,
};
pub use status_bar::{render_status_bar, StatusBarBufferInfo, StatusBarParams};
