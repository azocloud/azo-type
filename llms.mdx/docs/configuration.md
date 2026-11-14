# Configuration & Shortcuts (/docs/configuration)

Open the **Settings** sub-window via **File → Settings...** or `Ctrl+,` (`Cmd+,` on macOS). Preferences are persisted across restarts via `eframe` storage (`app/src/core/settings.rs`).

## Settings Tabs [#settings-tabs]

### 1. Appearance [#1-appearance]

- **Language:** Switch UI language at runtime (`English` or `Deutsch`), or follow the OS locale detected at startup via `sys-locale`.
- **Color Theme:** Choose **Dark** (`#1C1B22`), **Light** (`#FFFFFF`), or **System-Default** (tracks OS dark/light preference).
- **Editor Font Size:** Adjustable from `10.0 pt` to `22.0 pt` (default `13.5 pt`).
- **Word Wrap:** Automatically wrap long lines to the editor pane width with `↩` continuation markers.
- **Line Numbers Gutter:** Show or hide line numbers beside the editor.
- **Syntax Highlighting:** Toggle real-time Typst syntax highlighting powered by `typst-syntax`.

### 2. Editor & Filesystem [#2-editor--filesystem]

- **Auto-Save on Edit:** Automatically write modified disk buffers as you type (enabled by default).
- **Show Hidden Files:** Display dotfiles and hidden directories (such as `.git` or `.vscode`) in the sidebar file tree.
- **Active Project Folder:** Inspect the active workspace root, switch folders, or trigger a disk rescan.

### 3. Compiler & Preview [#3-compiler--preview]

- **Typst Compiler Version:** Use built-in `0.15.1` or download/activate legacy `0.14.2`, `0.13.1`, or `0.12.0` releases.
- **Auto-Compile on Type:** Incrementally recompile while editing (enabled by default).
- **Typing Debounce Interval:** Configurable from `50 ms` to `800 ms` (default `120 ms`).
- **Preview Rasterization Scale:** `1.5x Fast`, `2.0x Crisp` (default), or `3.0x Ultra HiDPI`.
- **Preview Interaction:** Toggle **Auto-Jump** source selection sync and **Hand Pan** mode.

### 4. Spell & Grammar [#4-spell--grammar]

- **Enable Spell & Grammar Checking:** Master toggle (disabled by default).
- **Auto-Check While Typing:** Debounced background checking (`150 ms`–`2000 ms`, default `450 ms`).
- **Grammar & Typography Rules:** Toggle `nlprule` and pure-Rust typography/repetition checks.
- **Document Language Override:** `Auto` (reads `#set text(lang: ...)` or runs Lingua detection) or explicit `en` / `de` / `fr` / `es`.
- **Dictionaries:** Download/remove Tier-1 language packs, import Tier-2 `.dic`/`.aff` files, and manage ignored words.

## Keyboard Shortcuts [#keyboard-shortcuts]

On macOS, `Ctrl` corresponds to `Cmd` (`Modifiers::COMMAND` in `app/src/platform/shortcuts.rs`):

| Action | Windows / Linux | macOS |
| :--- | :--- | :--- |
| New File in Project | `Ctrl + N` | `Cmd + N` |
| Open Project Folder | `Ctrl + O` | `Cmd + O` |
| Save Active File | `Ctrl + S` | `Cmd + S` |
| Export PDF | `Ctrl + E` | `Cmd + E` |
| Open Settings | `Ctrl + ,` | `Cmd + ,` |
| Toggle Left Sidebar | `Ctrl + B` | `Cmd + B` |
| Toggle Bottom Panel | `Ctrl + J` | `Cmd + J` |
| Rescan Folder on Disk | `F5` | `F5` |
| Undo / Redo (incl. CRDT Collab) | `Ctrl + Z` / `Ctrl + Y` or `Ctrl + Shift + Z` | `Cmd + Z` / `Cmd + Shift + Z` |
| Cut / Copy / Paste / Select All | `Ctrl + X` / `C` / `V` / `A` | `Cmd + X` / `C` / `V` / `A` |
| Zoom Preview Canvas | `Ctrl + Mouse Wheel` | `Cmd + Mouse Wheel` |

## Localization [#localization]

UI strings live in monolingual Weblate-compatible YAML files under `locales/` (`locales/en.yml`, `locales/de.yml`) powered by `rust-i18n`. New translations can be added by creating a matching `locales/<lang>.yml` file and registering it in `app/src/core/i18n.rs`.