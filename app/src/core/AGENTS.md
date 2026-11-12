# `app/src/core/` — Scoped Agent Rules

This directory contains pure domain logic for `typst-ide`. In addition to root [`AGENTS.md`](../../../AGENTS.md), these strict rules apply:

1. **Zero UI / Async / OS Dependencies:**
   - NEVER import `egui`, `eframe`, `rfd`, `muda`, `tokio`, `iroh`, `arboard`, or `reqwest` in `app/src/core/`.
   - Permitted external crates: `std`, `typst`, `typst_syntax`, `typst_pdf`.
2. **No Direct Filesystem I/O:**
   - Functions in `core/` must not call `std::fs::*`.
   - When file contents are needed (e.g., resolving `#include "chapter.typ"` in `outline.rs`), accept a `&dyn SourceResolver` trait implementation so callers can pass in-memory test doubles or `DiskProject`.
3. **Mandatory Documentation (`#![warn(missing_docs)]`):**
   - Every `pub` struct, struct field, enum, enum variant, trait, constant, and function in `app/src/core/` MUST have a `///` doc comment.
4. **Companion Unit Tests Only:**
   - Every `app/src/core/<name>.rs` declares `#[cfg(test)] mod tests;` and places all unit tests in `app/src/core/<name>/tests.rs`.
   - All tests must run purely in-memory in `< 10ms` without temporary directories or network sockets.
