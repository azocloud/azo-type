//! Unit tests for the Tier-1 dictionary cache and download status tracking.

use std::fs;

use uuid::Uuid;

use super::*;
use crate::core::spellcheck::find_tier1_pack;

#[test]
fn reports_installed_only_when_all_manifest_files_exist() {
    let dir = std::env::temp_dir().join(format!("azo-dict-cache-{}", Uuid::new_v4()));
    let mgr = DictionaryManager::with_dir(dir.clone());

    assert_eq!(mgr.pack_status("fr"), DictionaryPackStatus::NotInstalled);
    assert!(mgr.installed_paths("fr").is_none());

    let (_, pack) = find_tier1_pack("fr").expect("fr pack exists");
    let fr_dir = mgr.pack_dir("fr");
    fs::create_dir_all(&fr_dir).unwrap();
    for file in &pack.files {
        fs::write(fr_dir.join(&file.filename), b"stub").unwrap();
    }

    assert_eq!(mgr.pack_status("fr"), DictionaryPackStatus::Installed);
    assert!(mgr.installed_paths("fr").is_some());

    mgr.remove_pack("fr").expect("remove fr pack");
    assert_eq!(mgr.pack_status("fr"), DictionaryPackStatus::NotInstalled);

    let _ = fs::remove_dir_all(&dir);
}
