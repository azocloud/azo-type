# GOTCHAS.md — Non-Obvious Invariants, Pitfalls & Sensitive Areas

This document records architectural nuances, threading constraints, and subtle pitfalls in `azo-type` that are not obvious from signatures alone. Read this before modifying platform, collaboration, or compiler code.

---

## 1. Native File Dialogs (`rfd`) Must Never Block the egui Frame Loop

### The Pitfall
Calling `rfd::FileDialog::pick_folder()` or `rfd::AsyncFileDialog::new().pick_folder()` directly inside an `egui::App::update()` turn freezes the entire window. On Linux (Wayland/X11) and macOS, running a synchronous dialog inside the event loop can cause a deadlock or crash the display server connection.

### How It Works Here (`app/src/platform/dialogs.rs`)
Native file dialogs are spawned on a dedicated background thread:
```rust
pub fn spawn_open_folder_dialog(tx: Sender<IoEvent>, ctx: Context) {
    std::thread::spawn(move || {
        let opt = block_on_or_spin(async {
            rfd::AsyncFileDialog::new().pick_folder().await
        });
        if let Some(folder) = opt {
            let _ = tx.send(IoEvent::FolderOpened(folder.path().to_path_buf()));
            ctx.request_repaint(); // Wakes egui to process the IoEvent channel
        }
    });
}
```
**Rule:** Never await dialogs inside `AzoTypeApp::update`. Send a background worker and receive the result via `IoEvent` in `AzoTypeApp::poll_channels`.

---

## 2. The Three String Offset Systems (Byte vs. Char vs. UTF-16)

### The Pitfall
There are three completely different text offset coordinates in this codebase:
1. **UTF-8 Byte Indices:** Rust string slicing `&text[start_byte..end_byte]`. Panics if index does not align with a UTF-8 character boundary.
2. **Unicode Scalar Values (`char`):** Used by `egui::text_edit`, `CCursor`, and syntax highlighting. One emoji or accented character = 1 `char`.
3. **UTF-16 Code Units:** Used by `yrs` CRDT (`YText`). In UTF-16, characters in the Astral plane (such as emojis `🌍` or math symbols) count as **2 code units** (surrogate pairs).

Directly applying a `yrs` UTF-16 diff index to a Rust string slice or `egui` cursor will corrupt text or cause a panic when characters outside the Basic Multilingual Plane (BMP) are present.

### How It Works Here (`app/src/core/diff.rs`)
Always use the verified conversion functions:
- `char_to_utf16_offset(str, char_idx) -> u32`
- `utf16_to_char_offset(str, utf16_offset) -> usize`
- `compute_utf16_diff(old_text, new_text) -> Option<Utf16TextDiff>`

All conversions are thoroughly tested against astral plane emojis in `app/tests/diff_tests.rs`.

---

## 3. Headless Screenshot Mode & Sub-Windows (`embed_dialogs`)

### The Pitfall
In normal desktop execution, modal dialogs (`export_pdf`, `file_modals`, `settings`) render as real native OS sub-windows using:
```rust
ctx.show_viewport_immediate(viewport_id, viewport_builder, |ctx, _class| { ... });
```
However, in headless automated test mode (`cargo run -- --screenshot /tmp/out.png`), no display server is attached. Calling `show_viewport_immediate` in headless mode fails to capture the dialog or panics.

### How It Works Here
All dialog functions in `app/src/ui/dialogs/` accept an `embed_dialogs: bool` flag:
- When `false` (desktop mode): renders as a separate OS viewport.
- When `true` (headless or fallback mode): renders as an embedded `egui::Window` inside the main canvas.

**Rule:** When adding a new modal dialog, always implement the dual-mode pattern demonstrated in `app/src/ui/dialogs/export_pdf.rs`.

---

## 4. Tokio Runtime vs. Immediate-Mode UI Concurrency

### The Pitfall
`egui` runs on the main thread with a synchronous 60 FPS update loop. The P2P collaborative engine (`iroh` and `tokio`) requires an asynchronous multi-threaded runtime. Mixing them naively causes the UI to stutter or locks the Tokio worker threads.

### How It Works Here (`app/src/collab/session.rs`)
- `CollabSessionHandle` spawns its own dedicated Tokio multi-thread runtime on a background thread.
- Communication between the GUI and Tokio uses non-blocking channels:
  - `std::sync::mpsc::Sender<UiToSessionCmd>` sends user actions to Tokio.
  - `tokio::sync::mpsc::Sender<SessionToUiEvent>` sends remote updates back to egui.
  - Whenever an event is sent to the GUI, `ctx.request_repaint()` is called immediately to wake the next frame without polling delays.

---

## 5. Redacting Cryptographic Secrets in Collaboration Tickets

### The Pitfall
`SessionTicket` (`app/src/collab/protocol.rs`) contains peer public keys and secret relay routing tickets. Printing a raw ticket to stdout, error logs, or terminal traces can leak session secrets.

### How It Works Here
The `fmt::Debug` implementation for `SessionTicket` explicitly redacts sensitive bytes:
```rust
impl std::fmt::Debug for SessionTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionTicket")
            .field("endpoint_id", &self.endpoint_id)
            .field("ticket_secret", &"[REDACTED]")
            .finish()
    }
}
```

---

## 6. Incremental Typst Compilation & World Caching (`app/src/compiler.rs`)

### The Pitfall
Typst uses `comemo` to memoize pure evaluation queries across compile frames. If source files, fonts, or package files change on disk, but the `typst::World` (`EditorWorld`) fails to update the file revision or slot timestamp, `comemo` will serve stale, cached outputs.

### How It Works Here
- `EditorWorld` caches open buffer texts. Whenever `compile_project` is called, updated buffers are pushed to the world prior to invoking `typst::compile`.
- Dynamic packages from Typst Universe (`@preview/...`) are downloaded by `ReqwestDownloader` and cached in the system cache directory before compilation proceeds.

---

## 7. rust-i18n Monolingual YAML Files, Symlink Layout & Recompilation

### The Pitfall
1. `rust-i18n` v3 uses `_version: 1` for **monolingual** files named `<locale>.yml` (`en.yml`, `de.yml`). Using `_version: 2` instructs `rust-i18n` to interpret the YAML as a single multi-lingual dictionary where top-level keys are locale codes, corrupting key lookups.
2. `rust_i18n::i18n!("locales")` inside `app/src/lib.rs` looks for `app/locales`. The Weblate standard directory is at the repository root (`locales/`). A filesystem symlink `app/locales -> ../locales` ensures Weblate diffs repository root files while `cargo` locates them during compilation.
3. Editing `.yml` translation files does not always cause `cargo` to detect modified Rust source files. Always run `touch app/src/lib.rs` when editing YAML translations to force `rust-i18n` proc macros to re-embed updated strings.


