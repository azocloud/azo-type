//! Unit tests for cross-platform `MenuAction` variants and recent-project menu formatting.

use std::path::PathBuf;

use super::{MacOsMenuBar, MenuAction};
use crate::core::recent_projects::RecentProjects;

#[test]
fn recent_menu_actions_support_copy_and_equality() {
    let open = MenuAction::OpenRecentProject(3);
    let copied = open;
    assert_eq!(open, copied);
    assert_ne!(open, MenuAction::OpenRecentProject(4));
    assert_ne!(open, MenuAction::ClearRecentProjects);
    assert_eq!(
        MenuAction::ClearRecentProjects,
        MenuAction::ClearRecentProjects
    );
}

#[test]
fn recent_menu_labels_reuse_core_entry_formatter() {
    let path = PathBuf::from("/tmp/projects/thesis/");
    assert_eq!(
        RecentProjects::format_entry_label(&path),
        "thesis — /tmp/projects/thesis"
    );

    let root = PathBuf::from("/");
    assert_eq!(RecentProjects::format_entry_label(&root), "/");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn non_macos_menu_bar_stub_is_noop() {
    assert!(MacOsMenuBar::init().is_none());
    let mut bar = MacOsMenuBar;
    bar.update_recent_projects(&[PathBuf::from("/tmp/example")]);
    assert_eq!(bar.poll_action(), None);
}
