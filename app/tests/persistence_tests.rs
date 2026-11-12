use std::fs;

use azo_type::collab::doc::{CollabFileEntry, CollabProjectDoc};
use azo_type::collab::persistence::CollabPersistence;

#[test]
fn saves_and_reloads_crdt_snapshot_retaining_all_files() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_test_persist_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let persistence = CollabPersistence::new(&temp_dir);
    let mut doc = CollabProjectDoc::with_client_id(Some(100));
    doc.create_file(
        Some("11111111-1111-1111-1111-111111111111".to_string()),
        "main.typ",
        "= Main Document\n",
    );
    doc.create_file(
        Some("22222222-2222-2222-2222-222222222222".to_string()),
        "chapters/intro.typ",
        "== Intro\n",
    );

    persistence
        .save_snapshot(&doc)
        .expect("save snapshot should succeed");
    assert!(persistence.snapshot_path().is_file());
    assert!(persistence.file_ids_path().is_file());

    let mut fresh_doc = CollabProjectDoc::with_client_id(Some(200));
    persistence
        .load_and_reconcile_from_disk(&mut fresh_doc, false)
        .expect("load snapshot should succeed");

    let files = fresh_doc.list_files();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].resolved_path, "chapters/intro.typ");
    assert_eq!(files[0].content, "== Intro\n");
    assert_eq!(files[1].resolved_path, "main.typ");
    assert_eq!(files[1].content, "= Main Document\n");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn flushes_crdt_files_to_disk_with_parent_directories() {
    let temp_dir = std::env::temp_dir().join(format!("typst_test_flush_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let persistence = CollabPersistence::new(&temp_dir);
    let entries = vec![
        CollabFileEntry {
            file_id: "id-1".to_string(),
            raw_path: "subfolder/nested/test.typ".to_string(),
            resolved_path: "subfolder/nested/test.typ".to_string(),
            content: "Content in nested folder".to_string(),
        },
        CollabFileEntry {
            file_id: "id-2".to_string(),
            raw_path: "root.typ".to_string(),
            resolved_path: "root.typ".to_string(),
            content: "Content at root".to_string(),
        },
    ];

    let modified = persistence
        .flush_crdt_files_to_disk(&entries)
        .expect("flush should succeed");
    assert_eq!(modified.len(), 2);

    assert_eq!(
        fs::read_to_string(temp_dir.join("subfolder/nested/test.typ")).unwrap(),
        "Content in nested folder"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.join("root.typ")).unwrap(),
        "Content at root"
    );

    // Second flush with identical content should report 0 modified files
    let modified_again = persistence.flush_crdt_files_to_disk(&entries).unwrap();
    assert_eq!(modified_again.len(), 0);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn prevents_path_traversal_when_flushing_files() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_test_traversal_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();

    let persistence = CollabPersistence::new(&temp_dir);
    let escape_target = temp_dir.join("../escaped.txt");

    let malicious_entries = vec![CollabFileEntry {
        file_id: "bad-id".to_string(),
        raw_path: "../escaped.txt".to_string(),
        resolved_path: "../escaped.txt".to_string(),
        content: "Malicious payload".to_string(),
    }];

    let modified = persistence
        .flush_crdt_files_to_disk(&malicious_entries)
        .unwrap();
    assert_eq!(modified.len(), 0);
    assert!(!escape_target.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn scans_and_reconciles_new_disk_files_into_crdt() {
    let temp_dir =
        std::env::temp_dir().join(format!("typst_test_reconcile_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(temp_dir.join("chapters")).unwrap();
    fs::write(temp_dir.join("main.typ"), "Disk Main").unwrap();
    fs::write(temp_dir.join("chapters/section.typ"), "Disk Section").unwrap();

    let persistence = CollabPersistence::new(&temp_dir);
    let mut doc = CollabProjectDoc::new();

    persistence
        .load_and_reconcile_from_disk(&mut doc, true)
        .expect("reconcile should succeed");

    let files = doc.list_files();
    assert_eq!(files.len(), 2);

    let main_entry = files
        .iter()
        .find(|f| f.resolved_path == "main.typ")
        .unwrap();
    assert_eq!(main_entry.content, "Disk Main");

    let sec_entry = files
        .iter()
        .find(|f| f.resolved_path == "chapters/section.typ")
        .unwrap();
    assert_eq!(sec_entry.content, "Disk Section");

    let _ = fs::remove_dir_all(&temp_dir);
}
