//! Unit tests for `RecentProjects` MRU tracking, lexical normalization, and formatting.

use std::path::{Path, PathBuf};

use super::*;

#[test]
fn empty_recent_projects_starts_empty_and_ignores_blank_paths() {
    let mut recent = RecentProjects::new();
    assert!(recent.is_empty());
    assert_eq!(recent.entries(), &[] as &[PathBuf]);
    assert_eq!(recent.most_recent(), None);

    recent.record(PathBuf::from(""));
    recent.record(PathBuf::from("   "));
    assert!(recent.is_empty());
    assert_eq!(recent.most_recent(), None);
}

#[test]
fn from_paths_deduplicates_lexically_preserves_first_seen_order_and_caps_at_max() {
    let paths: Vec<PathBuf> = vec![
        PathBuf::from(""),
        PathBuf::from("/workspace/alpha/"),
        PathBuf::from("/workspace/./alpha"),
        PathBuf::from("/workspace/beta"),
        PathBuf::from("/workspace/sub/../beta/"),
        PathBuf::from("/workspace/gamma"),
        PathBuf::from("/workspace/delta"),
        PathBuf::from("/workspace/epsilon"),
        PathBuf::from("/workspace/zeta"),
        PathBuf::from("/workspace/eta"),
        PathBuf::from("/workspace/theta"),
        PathBuf::from("/workspace/iota"),
        PathBuf::from("/workspace/kappa"),
        PathBuf::from("/workspace/lambda"),
    ];

    let recent = RecentProjects::from_paths(paths);
    assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
    assert_eq!(recent.most_recent(), Some(Path::new("/workspace/alpha")));
    assert_eq!(
        recent.entries(),
        &[
            PathBuf::from("/workspace/alpha"),
            PathBuf::from("/workspace/beta"),
            PathBuf::from("/workspace/gamma"),
            PathBuf::from("/workspace/delta"),
            PathBuf::from("/workspace/epsilon"),
            PathBuf::from("/workspace/zeta"),
            PathBuf::from("/workspace/eta"),
            PathBuf::from("/workspace/theta"),
            PathBuf::from("/workspace/iota"),
            PathBuf::from("/workspace/kappa"),
        ]
    );
}

#[test]
fn record_reorders_existing_entry_to_front_and_caps_at_ten() {
    let mut recent = RecentProjects::new();
    for idx in 0..MAX_RECENT_PROJECTS {
        recent.record(PathBuf::from(format!("/projects/p{idx}")));
    }

    assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
    // Most recently recorded (`p9`) is at index 0, oldest (`p0`) is at index 9.
    assert_eq!(recent.most_recent(), Some(Path::new("/projects/p9")));
    assert_eq!(recent.entries()[9], PathBuf::from("/projects/p0"));

    // Re-recording an existing middle entry moves it to index 0 without evicting any entry.
    recent.record(PathBuf::from("/projects/p3/"));
    assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
    assert_eq!(recent.most_recent(), Some(Path::new("/projects/p3")));
    assert_eq!(recent.entries()[1], PathBuf::from("/projects/p9"));
    assert_eq!(recent.entries()[9], PathBuf::from("/projects/p0"));

    // Recording a brand new 11th entry evicts the oldest entry (`p0`).
    recent.record(PathBuf::from("/projects/p10"));
    assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
    assert_eq!(recent.most_recent(), Some(Path::new("/projects/p10")));
    assert!(!recent.entries().contains(&PathBuf::from("/projects/p0")));
}

#[test]
fn record_normalizes_separators_and_dot_segments_cross_platform() {
    let mut recent = RecentProjects::new();

    recent.record(PathBuf::from("/home/alex/papers/thesis///"));
    assert_eq!(
        recent.most_recent(),
        Some(Path::new("/home/alex/papers/thesis"))
    );

    // Dot segments resolve to the same entry and move/update it cleanly.
    recent.record(PathBuf::from("/home/alex/drafts/../papers/./thesis/"));
    assert_eq!(recent.entries().len(), 1);
    assert_eq!(
        recent.most_recent(),
        Some(Path::new("/home/alex/papers/thesis"))
    );

    // Windows backslash vs forward slash paths deduplicate lexically even on POSIX hosts.
    recent.record(PathBuf::from(r"C:\Users\Alice\Typst\book\"));
    assert_eq!(recent.entries().len(), 2);
    assert_eq!(
        recent.most_recent(),
        Some(Path::new(r"C:\Users\Alice\Typst\book"))
    );

    recent.record(PathBuf::from("c:/Users/Alice/Typst/./book/"));
    assert_eq!(recent.entries().len(), 2);
    assert_eq!(
        recent.most_recent(),
        Some(Path::new("c:/Users/Alice/Typst/book"))
    );

    // UNC paths normalize trailing separators cleanly.
    recent.record(PathBuf::from(r"\\fileserver\shared\docs\paper\"));
    assert_eq!(
        recent.most_recent(),
        Some(Path::new(r"\\fileserver\shared\docs\paper"))
    );

    // Relative paths strip leading `./` and resolve interior `..`.
    recent.record(PathBuf::from("./sub/../notes/"));
    assert_eq!(recent.most_recent(), Some(Path::new("notes")));
}

#[test]
fn remove_matches_lexically_and_clear_empties_list() {
    let mut recent = RecentProjects::from_paths([
        PathBuf::from("/work/first"),
        PathBuf::from(r"D:\Docs\Second\"),
        PathBuf::from("/work/third"),
    ]);

    recent.remove(Path::new(""));
    recent.remove(Path::new("/work/nonexistent"));
    assert_eq!(recent.entries().len(), 3);

    recent.remove(Path::new("D:/Docs/./Second/"));
    assert_eq!(
        recent.entries(),
        &[PathBuf::from("/work/first"), PathBuf::from("/work/third")]
    );

    recent.remove(Path::new("/work/other/../first/"));
    assert_eq!(recent.entries(), &[PathBuf::from("/work/third")]);

    recent.clear();
    assert!(recent.is_empty());
    assert_eq!(recent.most_recent(), None);
}

#[test]
fn serde_roundtrip_and_default_handling() {
    let mut recent = RecentProjects::new();
    recent.record(PathBuf::from("/work/alpha"));
    recent.record(PathBuf::from("/work/beta"));

    let json = serde_json::to_string(&recent).expect("serialize RecentProjects");
    let loaded: RecentProjects = serde_json::from_str(&json).expect("deserialize RecentProjects");
    assert_eq!(loaded, recent);

    let empty_obj: RecentProjects =
        serde_json::from_str("{}").expect("deserialize empty JSON object via serde(default)");
    assert!(empty_obj.is_empty());
}

#[test]
fn format_entry_label_formats_folders_and_root_fallbacks_cleanly() {
    assert_eq!(RecentProjects::format_entry_label(Path::new("")), "");
    assert_eq!(
        RecentProjects::format_entry_label(Path::new("/home/alex/thesis/")),
        "thesis — /home/alex/thesis"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"C:\Users\Alice\paper\")),
        r"paper — C:\Users\Alice\paper"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\server\share\report\")),
        r"report — \\server\share\report"
    );
    assert_eq!(RecentProjects::format_entry_label(Path::new("/")), "/");
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"C:\")),
        r"C:\"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\server\share")),
        r"\\server\share"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new("./single_dir/")),
        "single_dir"
    );
    assert_eq!(RecentProjects::format_entry_label(Path::new(".")), ".");
}

#[test]
fn windows_verbatim_drive_and_unc_prefixes_normalize_and_deduplicate() {
    let mut recent = RecentProjects::from_paths([
        PathBuf::from(r"\\?\C:\Users\Alice\paper\"),
        PathBuf::from(r"C:\Users\Alice\drafts\..\paper"),
        PathBuf::from(r"\\?\UNC\server\share\report\"),
        PathBuf::from(r"\\server\share\report"),
    ]);

    assert_eq!(
        recent.entries(),
        &[
            PathBuf::from(r"C:\Users\Alice\paper"),
            PathBuf::from(r"\\server\share\report"),
        ]
    );

    // Case-insensitive `unc\` and drive letters deduplicate and preserve clean normalized paths.
    recent.record(PathBuf::from(r"\\?\unc\server\share\other\..\report\"));
    assert_eq!(recent.entries().len(), 2);
    assert_eq!(
        recent.most_recent(),
        Some(Path::new(r"\\server\share\report"))
    );

    recent.record(PathBuf::from(r"\\?\c:\Users\Alice\paper\"));
    assert_eq!(recent.entries().len(), 2);
    assert_eq!(
        recent.most_recent(),
        Some(Path::new(r"c:\Users\Alice\paper"))
    );

    // Root verbatim paths strip `\\?\` / `\\?\UNC\` cleanly and format without verbatim prefix.
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\?\C:\")),
        r"C:\"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\?\C:\foo\")),
        r"foo — C:\foo"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\?\UNC\server\share")),
        r"\\server\share"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\?\UNC\server\share\")),
        r"\\server\share"
    );
    assert_eq!(
        RecentProjects::format_entry_label(Path::new(r"\\?\UNC\server\share\thesis\")),
        r"thesis — \\server\share\thesis"
    );

    // Removal works across verbatim and non-verbatim representations.
    recent.remove(Path::new(r"C:\Users\Alice\paper"));
    assert_eq!(recent.entries(), &[PathBuf::from(r"\\server\share\report")]);
    recent.remove(Path::new(r"\\?\UNC\server\share\report"));
    assert!(recent.is_empty());
}
