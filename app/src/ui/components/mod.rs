//! UI reusable panels, controls, and components.

pub mod editor;
pub mod menu_bar;
pub mod preview;
pub mod sidebar;
pub mod status_bar;

pub use editor::{render_editor_pane, EditorPaneOutput, EditorPaneParams};
pub use menu_bar::{render_desktop_menu_bar, EditorContextAction, MenuBarOutput, MenuBarParams};
pub use preview::{render_preview_pane, PreviewPaneParams};
pub use sidebar::{render_sidebar, SidebarOutput, SidebarParams};
pub use status_bar::{render_status_bar, StatusBarBufferInfo, StatusBarParams};
