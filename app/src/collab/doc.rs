use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use yrs::block::ClientID;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
    Any, Assoc, Doc, GetString, IndexedSequence, Map, MapRef, OffsetKind, Options, Origin, Out,
    ReadTxn, StateVector, StickyIndex, Text, TextRef, Transact, UndoManager, Update,
};

pub const LOCAL_ORIGIN: &str = "local";
pub const REMOTE_ORIGIN: &str = "remote";
pub const FILES_MAP_KEY: &str = "files";

/// Snapshot of a collaborative file entry inside the Yrs CRDT `"files"` map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollabFileEntry {
    pub file_id: String,
    pub raw_path: String,
    /// Deterministic collision-resolved path relative to the project root.
    pub resolved_path: String,
    pub content: String,
}

/// Minimal text edit operation expressed in `yrs` UTF-16 code-unit offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utf16TextDiff {
    pub utf16_index: u32,
    pub utf16_remove_len: u32,
    pub insert_text: String,
}

/// Cursor and selection range expressed in `egui` Unicode scalar `char` indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharSelection {
    pub primary_char: usize,
    pub secondary_char: usize,
}

impl CharSelection {
    pub fn caret(char_idx: usize) -> Self {
        Self {
            primary_char: char_idx,
            secondary_char: char_idx,
        }
    }
}

/// Converts an `egui` `char` index (Unicode scalar count) into a `yrs` UTF-16 code unit offset.
pub fn char_to_utf16_offset(s: &str, char_idx: usize) -> u32 {
    s.chars()
        .take(char_idx)
        .map(|c| c.len_utf16() as u32)
        .sum()
}

/// Converts a `yrs` UTF-16 code unit offset back into an `egui` `char` index (Unicode scalar count).
pub fn utf16_to_char_offset(s: &str, utf16_offset: u32) -> usize {
    let mut accumulated_utf16 = 0u32;
    let mut char_count = 0usize;
    for ch in s.chars() {
        if accumulated_utf16 >= utf16_offset {
            break;
        }
        accumulated_utf16 += ch.len_utf16() as u32;
        char_count += 1;
    }
    char_count
}

/// Computes the minimal single-span difference between `old_text` and `new_text` using
/// common prefix and common suffix over Unicode scalar values (`char`), and returns the
/// corresponding `yrs` UTF-16 offset, removal length, and inserted substring.
pub fn compute_utf16_diff(old_text: &str, new_text: &str) -> Option<Utf16TextDiff> {
    if old_text == new_text {
        return None;
    }

    let old_chars: Vec<char> = old_text.chars().collect();
    let new_chars: Vec<char> = new_text.chars().collect();

    let mut prefix_chars = 0usize;
    let min_len = old_chars.len().min(new_chars.len());
    while prefix_chars < min_len && old_chars[prefix_chars] == new_chars[prefix_chars] {
        prefix_chars += 1;
    }

    let mut old_suffix_end = old_chars.len();
    let mut new_suffix_end = new_chars.len();
    while old_suffix_end > prefix_chars
        && new_suffix_end > prefix_chars
        && old_chars[old_suffix_end - 1] == new_chars[new_suffix_end - 1]
    {
        old_suffix_end -= 1;
        new_suffix_end -= 1;
    }

    let utf16_index: u32 = old_chars[..prefix_chars]
        .iter()
        .map(|c| c.len_utf16() as u32)
        .sum();
    let utf16_remove_len: u32 = old_chars[prefix_chars..old_suffix_end]
        .iter()
        .map(|c| c.len_utf16() as u32)
        .sum();
    let insert_text: String = new_chars[prefix_chars..new_suffix_end].iter().collect();

    Some(Utf16TextDiff {
        utf16_index,
        utf16_remove_len,
        insert_text,
    })
}

/// Normalizes a relative path to forward slashes without leading slashes.
pub fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .trim()
        .to_string()
}

/// Deterministic collision suffix helper: when two `file_id`s claim the same relative path
/// concurrently, the lexicographically smallest `file_id` keeps the original path and any
/// subsequent `file_id` receives a deterministic `(conflict-<short_id>)` suffix before the extension.
pub fn format_collision_path(raw_path: &str, file_id: &str) -> String {
    let short_id: String = file_id.chars().take(6).collect();
    let path = Path::new(raw_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    let parent = path
        .parent()
        .and_then(|p| p.to_str())
        .filter(|p| !p.is_empty());

    let file_name = match ext {
        Some(e) => format!("{}.conflict-{}.{}", stem, short_id, e),
        None => format!("{}.conflict-{}", stem, short_id),
    };

    match parent {
        Some(p) => format!("{}/{}", p.replace('\\', "/"), file_name),
        None => file_name,
    }
}

/// High-level wrapper around a `yrs::Doc` representing a collaborative Typst project.
///
/// Schema:
/// `Map "files": file_id (UUID string) -> Map { "path": String, "text": Text, "deleted": bool }`
pub struct CollabProjectDoc {
    doc: Doc,
    files_map: MapRef,
    outgoing_updates: Arc<Mutex<Vec<Vec<u8>>>>,
    undo_managers: HashMap<String, UndoManager>,
}

impl CollabProjectDoc {
    pub fn new() -> Self {
        Self::with_client_id(None)
    }

    pub fn with_client_id(client_id: Option<u64>) -> Self {
        let mut opts = Options {
            offset_kind: OffsetKind::Utf16,
            ..Options::default()
        };
        if let Some(cid) = client_id {
            opts.client_id = ClientID::new(cid);
        }
        let doc = Doc::with_options(opts);
        let files_map = doc.get_or_insert_map(FILES_MAP_KEY);
        let outgoing_updates: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        let out_clone = Arc::clone(&outgoing_updates);
        let _ = doc.observe_update_v1("local_update_collector", move |txn, event| {
            let is_local = txn
                .origin()
                .map(|o| o == &Origin::from(LOCAL_ORIGIN))
                .unwrap_or(false);
            if is_local {
                if let Ok(mut queue) = out_clone.lock() {
                    queue.push(event.update.clone());
                }
            }
        });

        Self {
            doc,
            files_map,
            outgoing_updates,
            undo_managers: HashMap::new(),
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn doc_mut(&mut self) -> &mut Doc {
        &mut self.doc
    }

    pub fn client_id(&self) -> u64 {
        self.doc.client_id().get()
    }

    /// Drains all binary v1 updates generated by local transactions since the last call.
    pub fn drain_local_updates(&self) -> Vec<Vec<u8>> {
        if let Ok(mut queue) = self.outgoing_updates.lock() {
            std::mem::take(&mut *queue)
        } else {
            Vec::new()
        }
    }

    /// Encodes the local state vector as v1 bytes for the initial sync handshake.
    pub fn encode_state_vector_v1(&self) -> Vec<u8> {
        self.doc.transact().state_vector().encode_v1()
    }

    /// Encodes the full document state or diff from a peer's state vector as a v1 update.
    pub fn encode_diff_v1(&self, remote_sv_bytes: &[u8]) -> Result<Vec<u8>, String> {
        let sv = if remote_sv_bytes.is_empty() {
            StateVector::default()
        } else {
            StateVector::decode_v1(remote_sv_bytes).map_err(|e| format!("Invalid SV: {}", e))?
        };
        Ok(self.doc.transact().encode_state_as_update_v1(&sv))
    }

    /// Encodes the full document snapshot for local persistence.
    pub fn encode_full_snapshot_v1(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    /// Applies a remote binary v1 update using `REMOTE_ORIGIN` so it does NOT echo back to peers
    /// and does NOT pollute the local user's Undo stack.
    pub fn apply_remote_update_v1(&mut self, update_bytes: &[u8]) -> Result<(), String> {
        let update = Update::decode_v1(update_bytes)
            .map_err(|e| format!("Malformed Yrs update: {}", e))?;
        let mut txn = self.doc.transact_mut_with(Origin::from(REMOTE_ORIGIN));
        txn.apply_update(update)
            .map_err(|e| format!("Update apply error: {}", e))
    }

    /// Applies a remote binary v1 update while preserving the local user's cursor and selection
    /// in `active_file_id` via `StickyIndex`.
    pub fn apply_remote_update_preserving_cursor(
        &mut self,
        update_bytes: &[u8],
        active_file_id: Option<&str>,
        current_selection: Option<CharSelection>,
    ) -> Result<Option<CharSelection>, String> {
        let update = Update::decode_v1(update_bytes)
            .map_err(|e| format!("Malformed Yrs update: {}", e))?;

        let sticky_pair = if let (Some(fid), Some(sel)) = (active_file_id, current_selection) {
            self.capture_sticky_selection(fid, sel)
        } else {
            None
        };

        {
            let mut txn = self.doc.transact_mut_with(Origin::from(REMOTE_ORIGIN));
            txn.apply_update(update)
                .map_err(|e| format!("Update apply error: {}", e))?;
        }

        let restored = if let (Some(fid), Some((sticky_prim, sticky_sec))) =
            (active_file_id, sticky_pair)
        {
            self.resolve_sticky_selection(fid, sticky_prim, sticky_sec)
        } else {
            current_selection
        };

        Ok(restored)
    }

    fn capture_sticky_selection(
        &self,
        file_id: &str,
        sel: CharSelection,
    ) -> Option<(StickyIndex, StickyIndex)> {
        let txn = self.doc.transact();
        let file_map = self.get_file_map_read(&txn, file_id)?;
        let text_ref = file_map.get(&txn, "text")?.cast::<TextRef>().ok()?;
        let content = text_ref.get_string(&txn);

        let prim_utf16 = char_to_utf16_offset(&content, sel.primary_char);
        let sec_utf16 = char_to_utf16_offset(&content, sel.secondary_char);

        let sticky_prim = text_ref.sticky_index(&txn, prim_utf16, Assoc::After)?;
        let sticky_sec = text_ref.sticky_index(&txn, sec_utf16, Assoc::After)?;
        Some((sticky_prim, sticky_sec))
    }

    fn resolve_sticky_selection(
        &self,
        file_id: &str,
        sticky_prim: StickyIndex,
        sticky_sec: StickyIndex,
    ) -> Option<CharSelection> {
        let txn = self.doc.transact();
        let file_map = self.get_file_map_read(&txn, file_id)?;
        let text_ref = file_map.get(&txn, "text")?.cast::<TextRef>().ok()?;
        let content = text_ref.get_string(&txn);

        let prim_utf16 = sticky_prim.get_offset(&txn)?.index;
        let sec_utf16 = sticky_sec.get_offset(&txn)?.index;

        Some(CharSelection {
            primary_char: utf16_to_char_offset(&content, prim_utf16),
            secondary_char: utf16_to_char_offset(&content, sec_utf16),
        })
    }

    fn get_file_map_read<T: ReadTxn>(&self, txn: &T, file_id: &str) -> Option<MapRef> {
        match self.files_map.get(txn, file_id)? {
            Out::YMap(m) => Some(m),
            _ => None,
        }
    }

    fn ensure_undo_manager_for_file(&mut self, file_id: &str, text_ref: &TextRef) {
        if !self.undo_managers.contains_key(file_id) {
            let mut mgr = UndoManager::with_options(yrs::undo::Options::default());
            mgr.expand_scope(&self.doc, text_ref);
            mgr.include_origin(Origin::from(LOCAL_ORIGIN));
            self.undo_managers.insert(file_id.to_string(), mgr);
        }
    }

    /// Creates or updates a file in the CRDT with a specific `file_id` (or generates a new UUID v4).
    pub fn create_file(
        &mut self,
        file_id: Option<String>,
        rel_path: &str,
        initial_content: &str,
    ) -> String {
        let fid = file_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let normalized_path = normalize_rel_path(rel_path);

        let text_ref = {
            let mut txn = self.doc.transact_mut_with(Origin::from(LOCAL_ORIGIN));
            let file_map: MapRef = self.files_map.get_or_init(&mut txn, fid.as_str());
            file_map.insert(&mut txn, "path", Any::String(normalized_path.into()));
            file_map.insert(&mut txn, "deleted", Any::Bool(false));
            let text_ref: TextRef = file_map.get_or_init(&mut txn, "text");
            let existing = text_ref.get_string(&txn);
            if existing != initial_content {
                let len = char_to_utf16_offset(&existing, existing.chars().count());
                if len > 0 {
                    text_ref.remove_range(&mut txn, 0, len);
                }
                if !initial_content.is_empty() {
                    text_ref.insert(&mut txn, 0, initial_content);
                }
            }
            text_ref
        };

        self.ensure_undo_manager_for_file(&fid, &text_ref);
        fid
    }

    /// Applies a local string edit to `file_id` by computing the minimal UTF-16 diff and executing
    /// `remove_range` + `insert` inside a `LOCAL_ORIGIN` transaction.
    pub fn apply_local_text_edit(&mut self, file_id: &str, new_text: &str) -> bool {
        let current_text = match self.file_content(file_id) {
            Some(t) => t,
            None => return false,
        };

        let Some(diff) = compute_utf16_diff(&current_text, new_text) else {
            return false;
        };

        let text_ref_opt = {
            let mut txn = self.doc.transact_mut_with(Origin::from(LOCAL_ORIGIN));
            if let Some(Out::YMap(file_map)) = self.files_map.get(&txn, file_id) {
                let text_ref: TextRef = file_map.get_or_init(&mut txn, "text");
                if diff.utf16_remove_len > 0 {
                    text_ref.remove_range(&mut txn, diff.utf16_index, diff.utf16_remove_len);
                }
                if !diff.insert_text.is_empty() {
                    text_ref.insert(&mut txn, diff.utf16_index, &diff.insert_text);
                }
                Some(text_ref)
            } else {
                None
            }
        };

        if let Some(text_ref) = text_ref_opt {
            self.ensure_undo_manager_for_file(file_id, &text_ref);
            true
        } else {
            false
        }
    }

    /// Renames/moves a file by updating its `"path"` field under its stable `file_id` UUID key.
    pub fn rename_file(&mut self, file_id: &str, new_rel_path: &str) -> bool {
        let normalized = normalize_rel_path(new_rel_path);
        let mut txn = self.doc.transact_mut_with(Origin::from(LOCAL_ORIGIN));
        if let Some(Out::YMap(file_map)) = self.files_map.get(&txn, file_id) {
            file_map.insert(&mut txn, "path", Any::String(normalized.into()));
            true
        } else {
            false
        }
    }

    /// Marks a file as deleted in the `"files"` CRDT map.
    pub fn delete_file(&mut self, file_id: &str) -> bool {
        let mut txn = self.doc.transact_mut_with(Origin::from(LOCAL_ORIGIN));
        if let Some(Out::YMap(file_map)) = self.files_map.get(&txn, file_id) {
            file_map.insert(&mut txn, "deleted", Any::Bool(true));
            true
        } else {
            false
        }
    }

    /// Reverts the last local edit on `file_id` using `yrs::UndoManager` (skipping remote edits).
    pub fn undo_file(&mut self, file_id: &str) -> bool {
        if let Some(mgr) = self.undo_managers.get_mut(file_id) {
            mgr.undo_blocking()
        } else {
            false
        }
    }

    /// Redoes the last undone local edit on `file_id` using `yrs::UndoManager`.
    pub fn redo_file(&mut self, file_id: &str) -> bool {
        if let Some(mgr) = self.undo_managers.get_mut(file_id) {
            mgr.redo_blocking()
        } else {
            false
        }
    }

    /// Reads the current content of a single file by `file_id`.
    pub fn file_content(&self, file_id: &str) -> Option<String> {
        let txn = self.doc.transact();
        let file_map = self.get_file_map_read(&txn, file_id)?;
        if let Some(Out::Any(Any::Bool(true))) = file_map.get(&txn, "deleted") {
            return None;
        }
        let text_ref = file_map.get(&txn, "text")?.cast::<TextRef>().ok()?;
        Some(text_ref.get_string(&txn))
    }

    /// Finds the `file_id` corresponding to a resolved relative path.
    pub fn find_file_id_by_path(&self, rel_path: &Path) -> Option<String> {
        let target = normalize_rel_path(&rel_path.to_string_lossy());
        self.list_files()
            .into_iter()
            .find(|f| f.resolved_path == target || f.raw_path == target)
            .map(|f| f.file_id)
    }

    /// Returns all active (non-deleted) files in the CRDT document with deterministic
    /// path collision resolution.
    pub fn list_files(&self) -> Vec<CollabFileEntry> {
        let txn = self.doc.transact();
        let mut raw_entries: Vec<(String, String, String)> = Vec::new();

        for (fid_key, val) in self.files_map.iter(&txn) {
            let Out::YMap(file_map) = val else {
                continue;
            };
            if let Some(Out::Any(Any::Bool(true))) = file_map.get(&txn, "deleted") {
                continue;
            }
            let raw_path = match file_map.get(&txn, "path") {
                Some(Out::Any(Any::String(s))) => normalize_rel_path(&s),
                _ => match file_map.to_json(&txn) {
                    Any::Map(m) => match m.get("path") {
                        Some(Any::String(s)) => normalize_rel_path(s),
                        _ => continue,
                    },
                    _ => continue,
                },
            };
            let content = file_map
                .get(&txn, "text")
                .and_then(|o| o.cast::<TextRef>().ok())
                .map(|t| t.get_string(&txn))
                .unwrap_or_default();

            raw_entries.push((fid_key.to_string(), raw_path, content));
        }

        // Sort deterministically by file_id so collision resolution is identical on all peers!
        raw_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut path_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut resolved = Vec::with_capacity(raw_entries.len());

        for (file_id, raw_path, content) in raw_entries {
            let count = path_counts.entry(raw_path.clone()).or_insert(0);
            let resolved_path = if *count == 0 {
                raw_path.clone()
            } else {
                format_collision_path(&raw_path, &file_id)
            };
            *count += 1;

            resolved.push(CollabFileEntry {
                file_id,
                raw_path,
                resolved_path,
                content,
            });
        }

        resolved.sort_by(|a, b| a.resolved_path.cmp(&b.resolved_path));
        resolved
    }
}

impl Default for CollabProjectDoc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_docs_converge_with_swapped_update_order() {
        let mut peer_a = CollabProjectDoc::with_client_id(Some(101));
        let mut peer_b = CollabProjectDoc::with_client_id(Some(202));

        // Initial shared file created on Peer A and synced to Peer B
        let fid = peer_a.create_file(
            Some("11111111-1111-1111-1111-111111111111".to_string()),
            "main.typ",
            "= Title\n\nHello world!\n",
        );
        for u in peer_a.drain_local_updates() {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }
        assert_eq!(peer_a.file_content(&fid), peer_b.file_content(&fid));

        // Concurrent edits on both peers
        peer_a.apply_local_text_edit(&fid, "= Title (Edited by A)\n\nHello world!\n");
        peer_b.apply_local_text_edit(&fid, "= Title\n\nHello collaborative world from B!\n");

        let updates_from_a = peer_a.drain_local_updates();
        let updates_from_b = peer_b.drain_local_updates();

        // Clone into a third replica to test applying (B then A) vs (A then B)
        let mut peer_c = CollabProjectDoc::with_client_id(Some(303));
        peer_c
            .apply_remote_update_v1(&peer_a.encode_full_snapshot_v1())
            .unwrap();

        for u in &updates_from_b {
            peer_a.apply_remote_update_v1(u).unwrap();
            peer_c.apply_remote_update_v1(u).unwrap();
        }
        for u in &updates_from_a {
            peer_b.apply_remote_update_v1(u).unwrap();
        }

        assert_eq!(peer_a.file_content(&fid), peer_b.file_content(&fid));
        assert_eq!(peer_a.file_content(&fid), peer_c.file_content(&fid));
    }

    #[test]
    fn test_unicode_umlauts_emoji_and_multi_codepoint_offsets() {
        let mut doc = CollabProjectDoc::with_client_id(Some(1));
        let initial = "Grüße 🦀 aus München 👨‍👩‍👧‍👦 Ende";
        let fid = doc.create_file(None, "unicode.typ", initial);

        // Verify roundtrip between char index and UTF-16 offset for every char boundary
        let char_count = initial.chars().count();
        for c_idx in 0..=char_count {
            let utf16 = char_to_utf16_offset(initial, c_idx);
            let back = utf16_to_char_offset(initial, utf16);
            assert_eq!(back, c_idx, "Mismatch at char index {}", c_idx);
        }

        // Edit right after the crab emoji 🦀 and before the family emoji 👨‍👩‍👧‍👦
        let updated = "Grüße 🦀 & ÄÖÜß aus München 👨‍👩‍👧‍👦 Ende!";
        assert!(doc.apply_local_text_edit(&fid, updated));
        assert_eq!(doc.file_content(&fid).as_deref(), Some(updated));
    }

    #[test]
    fn test_sticky_cursor_preservation_on_remote_edit() {
        let mut peer_a = CollabProjectDoc::with_client_id(Some(10));
        let mut peer_b = CollabProjectDoc::with_client_id(Some(20));

        let initial = "Line 1\nTarget Word\nLine 3";
        let fid = peer_a.create_file(
            Some("22222222-2222-2222-2222-222222222222".to_string()),
            "chapters/intro.typ",
            initial,
        );
        for u in peer_a.drain_local_updates() {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }

        // Peer B has its cursor placed right before 'W' in "Target Word" (char index 14)
        let cursor_before = CharSelection::caret(14);
        assert_eq!(
            peer_b
                .file_content(&fid)
                .unwrap()
                .chars()
                .nth(cursor_before.primary_char),
            Some('W')
        );

        // Peer A inserts "Inserted Header 🦀\n" at the very top of the file
        let inserted_prefix = "Inserted Header 🦀\n";
        peer_a.apply_local_text_edit(&fid, &format!("{}{}", inserted_prefix, initial));
        let updates = peer_a.drain_local_updates();

        let mut cursor_after = Some(cursor_before);
        for u in updates {
            cursor_after = peer_b
                .apply_remote_update_preserving_cursor(&u, Some(&fid), cursor_after)
                .unwrap();
        }

        let resolved_idx = cursor_after.unwrap().primary_char;
        let final_text = peer_b.file_content(&fid).unwrap();
        assert_eq!(final_text.chars().nth(resolved_idx), Some('W'));
        assert_eq!(
            resolved_idx,
            cursor_before.primary_char + inserted_prefix.chars().count()
        );
    }

    #[test]
    fn test_concurrent_file_rename_and_path_collision_resolution() {
        let mut peer_a = CollabProjectDoc::with_client_id(Some(1));
        let mut peer_b = CollabProjectDoc::with_client_id(Some(2));

        // Both peers concurrently create a file named "notes.typ" with different UUIDs
        let id_a = peer_a.create_file(
            Some("aaaa0000-0000-0000-0000-000000000001".to_string()),
            "notes.typ",
            "Notes from A",
        );
        let id_b = peer_b.create_file(
            Some("bbbb0000-0000-0000-0000-000000000002".to_string()),
            "notes.typ",
            "Notes from B",
        );

        let up_a = peer_a.drain_local_updates();
        let up_b = peer_b.drain_local_updates();

        for u in up_a {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }
        for u in up_b {
            peer_a.apply_remote_update_v1(&u).unwrap();
        }

        let files_a = peer_a.list_files();
        let files_b = peer_b.list_files();
        assert_eq!(files_a, files_b);
        assert_eq!(files_a.len(), 2);
        assert_eq!(files_a[0].resolved_path, "notes.conflict-bbbb00.typ");
        assert_eq!(files_a[1].resolved_path, "notes.typ");

        // Now Peer A renames its file while Peer B edits it concurrently
        peer_a.rename_file(&id_a, "chapters/notes-a.typ");
        peer_b.apply_local_text_edit(&id_a, "Notes from A (updated by B during rename!)");

        let up_a2 = peer_a.drain_local_updates();
        let up_b2 = peer_b.drain_local_updates();
        for u in up_a2 {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }
        for u in up_b2 {
            peer_a.apply_remote_update_v1(&u).unwrap();
        }

        assert_eq!(peer_a.list_files(), peer_b.list_files());
        assert_eq!(
            peer_a.file_content(&id_a).as_deref(),
            Some("Notes from A (updated by B during rename!)")
        );
        let _ = id_b;
    }

    #[test]
    fn test_local_undo_only_reverts_own_changes() {
        let mut peer_a = CollabProjectDoc::with_client_id(Some(11));
        let mut peer_b = CollabProjectDoc::with_client_id(Some(22));

        let fid = peer_a.create_file(
            Some("33333333-3333-3333-3333-333333333333".to_string()),
            "main.typ",
            "Base",
        );
        for u in peer_a.drain_local_updates() {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }

        // Peer A makes a local edit
        peer_a.apply_local_text_edit(&fid, "Base + LocalA");
        for u in peer_a.drain_local_updates() {
            peer_b.apply_remote_update_v1(&u).unwrap();
        }

        // Peer B appends a remote edit at the end
        peer_b.apply_local_text_edit(&fid, "Base + LocalA + RemoteB");
        for u in peer_b.drain_local_updates() {
            peer_a.apply_remote_update_v1(&u).unwrap();
        }
        assert_eq!(
            peer_a.file_content(&fid).as_deref(),
            Some("Base + LocalA + RemoteB")
        );

        // Peer A hits Undo: only " + LocalA" should be undone, preserving " + RemoteB"!
        assert!(peer_a.undo_file(&fid));
        assert_eq!(
            peer_a.file_content(&fid).as_deref(),
            Some("Base + RemoteB")
        );
    }
}
