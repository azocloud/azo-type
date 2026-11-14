# Features (/docs/features)

## Live Preview & Interactive Source Sync [#live-preview--interactive-source-sync]

`azo-type` renders compiled Typst pages to RGBA textures using `typst_render` on a non-blocking worker thread:

- **Rasterization DPI:** Choose between **1.5x Fast**, **2.0x Crisp** (default), and **3.0x Ultra HiDPI** in Settings.
- **Zoom & Navigation:** Toggle **Fit Width**, select preset zoom percentages (`50%`–`200%`), or hold `Ctrl` + mouse wheel over the preview (`25%`–`400%`).
- **Click & Drag Text Selection:** Select text directly on rendered preview pages to copy it or automatically jump the editor cursor back to the corresponding Typst source range (**Auto-Jump**).
- **Hand Pan Mode:** Switch to **Hand Pan** in the preview toolbar to drag-pan across zoomed pages without triggering text selection.

## Multi-Version Typst Compiler Engine [#multi-version-typst-compiler-engine]

While Typst `0.15.1` is compiled directly into `azo-type`, older documents may target earlier Typst releases. In **Settings → Compiler & Preview** (or by clicking the Typst version badge in the status bar), you can switch between:

- **Typst `0.15.1`** (built-in offline default)
- **Legacy Typst `0.14.2`, `0.13.1`, and `0.12.0`** (downloaded on demand from official Typst GitHub releases for your host triple and verified against embedded SHA-256 checksums before activation)

## Hierarchical AST Document Outline [#hierarchical-ast-document-outline]

The outline parser (`app/src/core/outline.rs`) inspects the Typst AST (`typst-syntax`) rather than naive regexes:

- Extracts `= ...` markup headings and `#heading(level: ...)[...]` calls while ignoring headings inside comments, raw code blocks, and `show heading.where(...)` styling rules.
- Recursively resolves `#include "..."` and `#module` imports across project files and merges compiled outline items from templates.
- Also parses Markdown `# Heading` sections and BibTeX `@article{key, ...}` citation entries.
- Assigns hierarchical numbering (`1`, `1.1`, `1.1.1`) and highlights the heading matching your current cursor position.

## Zero-Server P2P Collaboration [#zero-server-p2p-collaboration]

Open **Project → Collaboration (P2P)...** to co-author an entire project folder in real time without central servers:

- **CRDT Synchronization (`yrs`):** Multi-file project state is synced via Yjs CRDT updates (`Origin::from("local")` vs `Origin::from("remote")`), preserving cursor positions with `StickyIndex` and isolating each peer's undo/redo history (`yrs::UndoManager`).
- **Direct QUIC Transport (`iroh`):** Hosts bind an `iroh` QUIC endpoint (`b"azo-type/collab/v1"`) and generate a 256-bit secret invite ticket prefixed with `typst-collab-v1:`.
- **LAN / Localhost Only Mode:** Optionally disable public relays when collaborating on a trusted local network.
- **Peer Presence & Persistence:** Displays connected peers with custom display names, cursor colors, active file indicators, and connection routes (`Direct` vs `Relay` with RTT). Snapshots are saved to `.typst-collab/` with path-traversal protection and deterministic conflict naming (`<stem>.conflict-<id>.<ext>`).

<Callout type="warn" title="Invite Ticket Security">
Treat `typst-collab-v1:` invite tickets like passwords—anyone holding a ticket can join the active P2P session. Use the **Mask Ticket** toggle when screen-sharing.
</Callout>

## Standards-Compliant PDF Export [#standards-compliant-pdf-export]

Press `Ctrl+E` (`Cmd+E` on macOS) or choose **File → Export PDF...** to configure standards-checked PDF output:

- **PDF Versions:** `1.4`, `1.5`, `1.6`, `1.7` (default), and `2.0`.
- **PDF/A Archival Standards (ISO 19005):** `PDF/A-1b`, `PDF/A-1a`, `PDF/A-2b`, `PDF/A-2u`, `PDF/A-2a`, `PDF/A-3b`, `PDF/A-3u`, `PDF/A-3a`, `PDF/A-4`, `PDF/A-4f`, and `PDF/A-4e`.
- **Accessibility:** Toggle **PDF/UA-1** (ISO 14289-1) and **Tagged PDF** structure tree generation.
- **Compatibility Guard:** Automatically validates combinations (for example, `PDF/A-1` requires `PDF 1.4` and `PDF/A-4` requires `PDF 2.0`), offers a one-click compatible version switch, and displays the equivalent `typst compile --pdf-standard` CLI flags.

## Offline Spell & Grammar Checking [#offline-spell--grammar-checking]

Disabled by default so it stays out of your way until needed, offline proofreading runs strictly locally inside **azo-type**:

- **Typst-Aware Prose Extraction:** Masks code, math, labels, comments, and syntax tokens before checking prose.
- **Tier-1 Language Packs (`en`, `de`, `fr`, `es`):** Download Hunspell dictionaries (`spellbook`) and `nlprule` grammar binaries on demand with SHA-256 verification. Language detection (`lingua`) or `#set text(lang: "...")` automatically selects the active language when set to `Auto`.
- **Tier-2 Custom Hunspell Dictionaries:** Import any local `.dic` / `.aff` dictionary pair from disk (automatically transcoded to UTF-8).
- **Interactive Diagnostics:** Review spelling, grammar, and typography suggestions in the bottom panel, jump directly to line/column, or add terms to your ignore list.