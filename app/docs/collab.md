# P2P Real-Time Collaborative Editing Architecture

## 1. Overview

Typst Studio includes built-in peer-to-peer (P2P) real-time collaborative editing across an entire Typst project folder on disk (`Linux`, `macOS`, and `Windows`). Collaboration is 100% optional: the IDE works completely offline without any network access by default, and requires no custom backend server.

Two or more collaborators can have **different files open simultaneously** (e.g., Peer A editing `main.typ` while Peer B edits `chapters/intro.typ` or `template.typ`) and all edits, file creations, renames, deletions, and live Typst document previews synchronize automatically across all peers.

---

## 2. Module Responsibilities

| Module | File | Responsibility |
| :--- | :--- | :--- |
| **CRDT Document & Diffing** | `src/collab/doc.rs` | `CollabProjectDoc` wrapping `yrs::Doc` (`OffsetKind::Utf16`), UUID-keyed `"files"` map, UTF-16 <-> `egui` `char` offset conversion, `StickyIndex` cursor preservation, deterministic path collision resolution, and per-file `yrs::UndoManager`. |
| **Awareness & Presence** | `src/collab/awareness.rs` | `CollabAwareness` wrapping `yrs::sync::Awareness` for peer display name, RGB color, active file path, and cursor/selection char offsets. |
| **Transport Trait & Iroh P2P** | `src/collab/transport.rs` | Narrow `CollabTransport` trait (`send_to_peer`, `broadcast`, `next_event`, `disconnect_all`) and `IrohTransport` implementation using `iroh = "=1.4.1"` QUIC connections, ticket generation/parsing, constant-time secret verification, 8 MiB frame size enforcement, and `Direct` vs. `Relay` path inspection. |
| **Disk & CRDT Persistence** | `src/collab/persistence.rs` | Saves and loads full `yrs` v1 binary state snapshots in `<project_root>/.typst-collab/crdt-snapshot.yrs` and flushes remote CRDT file updates directly into the project folder on disk. |
| **Background Session Actor** | `src/collab/session.rs` | Dedicated background `tokio` thread owning the `CollabProjectDoc`, `CollabAwareness`, and `CollabPersistence` without shared mutexes; handles sync protocol (`SyncStep1`/`SyncStep2`), multi-peer star/mesh forwarding, and exponential-backoff auto-reconnect. |
| **Collaboration UI & Overlays** | `src/collab/ui.rs` | `egui` Collaboration window (`Project > Collaboration (P2P)...`), invite ticket masking/copying, peer list with `Direct (QUIC)` / `Relay` badges, and `paint_remote_cursors_in_editor` for rendering colored remote cursors, selections, and name tags inside `egui::TextEdit`. |

---

## 3. Why Actor Ownership Instead of `Arc<Mutex<Doc>>`

We chose a **dedicated background Actor (`SessionActor` in `src/collab/session.rs`)** that exclusively owns the `yrs::Doc`, `yrs::sync::Awareness`, and `CollabPersistence` on a background `tokio` runtime thread, communicating with the `egui` UI thread strictly through non-blocking channels (`tokio::sync::mpsc::UnboundedSender<UiToSessionCmd>` and `std::sync::mpsc::Receiver<SessionToUiEvent>`):

1. **Zero UI Thread Blocking (60+ FPS Guarantee):** `egui` runs an immediate-mode render loop where any lock contention on a shared `Arc<Mutex<Doc>>` during network bursts or disk snapshots would stall frame rendering.
2. **Elimination of Re-Entrant Deadlocks:** `yrs` transaction observers (`observe_update_v1`) run synchronously during `TransactionMut` commit. Combining `Arc<Mutex<Doc>>` with async network callbacks and UI event handlers is a classic source of lock-order inversion. With single-actor ownership, all CRDT transactions execute sequentially on the actor task.
3. **Instant Wakeup:** Whenever a remote CRDT update or Awareness update arrives, the actor applies it, writes updated files to disk, emits `SessionToUiEvent`, and calls `egui::Context::request_repaint()`.

---

## 4. CRDT Schema & Offset Semantics

One `yrs::Doc` (configured with `OffsetKind::Utf16`) represents the entire project session:

```text
Root Map "files":
  "<file_uuid>" (UUID v4 string) -> yrs::Map {
      "path":    String (normalized relative path, e.g. "chapters/intro.typ"),
      "text":    yrs::Text (collaborative rich text buffer),
      "deleted": Bool (tombstone flag for propagating file deletion)
  }
```

- **Stable UUID Keys:** Files are keyed by immutable UUID v4 strings rather than file paths. Concurrent editing while a file is renamed or moved updates `"path"` on the same UUID entry without losing concurrent text edits.
- **Deterministic Path Collision Resolution:** If two peers concurrently create files with the same `"path"`, `CollabProjectDoc::all_files()` sorts colliding UUIDs lexicographically; the first keeps `<stem>.<ext>` and subsequent entries receive `<stem>.conflict-<short_uuid>.<ext>`.
- **UTF-16 vs. `egui` `char` Offsets:** `egui::TextEdit` indexes cursors by Unicode scalar values (`char`), whereas `yrs` (`OffsetKind::Utf16`) uses UTF-16 code units. `char_to_utf16_offset` and `utf16_to_char_offset` convert accurately across astral-plane characters (emoji like `🦀`, mathematical symbols, umlauts `äöü`).
- **No Echo (`Origin` Filtering):** Local edits commit with `Origin::from("local")`, while incoming remote updates and initial disk seeding commit with `Origin::from("remote")`. Only `"local"` transactions emit `CrdtUpdate` frames to peers.
- **Cursor Stability (`StickyIndex`) & Local Undo (`UndoManager`):** When applying a remote CRDT update to the currently open file, `apply_remote_update_preserving_cursor` attaches `yrs::StickyIndex` anchors (`Assoc::Before`) to the user's primary and secondary selection prior to applying the remote update, then restores the adjusted `CCursorRange` on `egui::TextEdit`. Per-file `yrs::UndoManager` instances track only `Origin::from("local")`, ensuring `Ctrl+Z` / `Ctrl+Y` never undoes a collaborator's edits.

---

## 5. Wire Protocol, Security & Persistence

```text
Peer A (Host)                                       Peer B (Joiner)
     |                                                     |
     |--- Share Ticket (typst-collab-v1:<base64-json>) --->|
     |<============ QUIC / TLS 1.3 via iroh 1.4.1 ========>|
     |<-- SyncStep1 { protocol_version: 1, secret, sv } ---|
     |--- SyncStep2 { update: encode_diff_v1(sv) } ------->|
     |--- SyncStep1 { protocol_version: 1, secret, sv } -->|
     |<-- SyncStep2 { update: encode_diff_v1(sv) } --------|
     |<== Incremental CrdtUpdate & AwarenessUpdate =======>|
```

- **Ticket Security:** The invite ticket (`SessionTicket`) contains the protocol version (`1`), a 256-bit random session secret, and the host's `EndpointAddr`. Its `Debug` implementation redacts the secret (`<redacted-secret>`), the UI masks the ticket string until explicitly revealed, and verification uses constant-time comparison (`constant_time_eq_hex`).
- **Framing & Limits:** Every QUIC bi-stream message is length-prefixed (`u32` big-endian) and capped at `MAX_WIRE_FRAME_BYTES` (8 MiB).
- **Disk Synchronization & `.typst-collab/`:** Joining or receiving remote CRDT updates writes updated project files directly to the project folder on disk (with path traversal checks preventing writes outside `project_root`) and persists the full CRDT state to `.typst-collab/crdt-snapshot.yrs` so offline edits merge cleanly upon reconnection.

---

## 6. Known Limitations

- Binary assets (e.g., `.png`, `.jpg`) are read from disk for local compilation; real-time CRDT synchronization targets UTF-8 text files (`.typ`, `.bib`, `.svg`, `.md`, `.txt`, `.toml`, `.json`).
- Multi-peer sessions use a host-star topology where the host forwards incremental CRDT and Awareness frames between connected peers (designed for 2–8 concurrent collaborators). If the host closes the session, peers can immediately start a new host session from their locally persisted `.typst-collab/crdt-snapshot.yrs` state.
