# Manual GUI Smoke Test Checklist

This manual smoke testing checklist is derived from `docs/FEATURES.md` and provides an end-to-end verification protocol for desktop GUI interactions across Linux, macOS, and Windows.

---

## Smoke Test Protocol

### 1. Application Launch & Workspace Initialization
- [ ] **Launch default app**: Run `cargo run`.
  - [ ] App launches with inner window size 1360x860.
  - [ ] Window title reads: `"Azo Type — Native Desktop Typst IDE"`.
  - [ ] Default project `./sample-project` opens automatically.
  - [ ] `main.typ` is loaded in editor, tabs show `main.typ`, `template.typ`, `chapters/intro.typ`, `chapters/methods.typ`.
  - [ ] Document compiles in background; preview pane renders formatted pages.
  - [ ] Status bar shows: `Compiled (<N> ms) | Main: main.typ | Pages: <P>`.

### 2. File Tree & Project Management
- [ ] **Expand / Collapse folders**: Click `chapters/` and `assets/` in sidebar tree.
- [ ] **Open file**: Click `refs.bib`.
  - [ ] `refs.bib` opens in editor with `[B]` icon.
  - [ ] Tab appears in the top tab bar.
- [ ] **Create new file**:
  - [ ] Click `+ File` in sidebar.
  - [ ] "New File in Project" window appears.
  - [ ] Enter `chapters/summary.typ` and press Enter / click "Create on Disk".
  - [ ] File is created on disk, opens in editor, and seeds default template text.
- [ ] **Rename file**:
  - [ ] Right-click `chapters/summary.typ` -> "Rename File...".
  - [ ] Change to `chapters/conclusion.typ` and click "Rename on Disk".
  - [ ] File name updates in sidebar, tab bar, and filesystem.
- [ ] **Delete file**:
  - [ ] Right-click `chapters/conclusion.typ` -> "Delete File".
  - [ ] File is removed from disk and tab bar. Active tab reverts to remaining open file.

### 3. Editor, Syntax Highlighting & Line Numbers
- [ ] **Type Typst markup**:
  - [ ] Type `= Test Heading`, `*bold text*`, `_italic_`, `$ x^2 + y^2 = z^2 $`.
  - [ ] Check syntax colors: keywords (blue/red), strings (green), math/operators (orange/purple).
- [ ] **Line numbers & continuation glyphs**:
  - [ ] Verify line numbers gutter on the left.
  - [ ] Enter a long paragraph: verify wrapped rows show `↪` in gutter and `↩` at line end.
- [ ] **Active line highlight**:
  - [ ] Move cursor up/down; verify full-width row highlight follows cursor.
- [ ] **Snippet toolbar**:
  - [ ] Click `= Heading`, `*Bold*`, `_Italic_`, `$ Math $`, `+ Table`, `+ Figure`.
  - [ ] Verify snippets are appended or wrap current selection.
- [ ] **Context menu**:
  - [ ] Select text and right-click -> "Bold (*...*)": wraps selected text.
  - [ ] Right-click -> "Copy", move cursor, right-click -> "Paste": text pasted accurately.
  - [ ] Test `Ctrl+Z` (Undo) and `Ctrl+Y` (Redo).

### 4. Document Outline Navigation
- [ ] **Live outline update**:
  - [ ] Observe Document Outline panel in left sidebar.
  - [ ] Verify hierarchy badges (`= 1`, `== 1.1`, `=== 1.1.1`).
  - [ ] Type a new heading `== Live Section`: outline updates immediately.
- [ ] **Jump navigation**:
  - [ ] Click an outline heading in an external file (e.g. `chapters/intro.typ`).
  - [ ] Editor opens `chapters/intro.typ` and places caret at target heading line.

### 5. Live Document Preview & Compiler
- [ ] **Debounced compilation**:
  - [ ] Type edits in editor: status bar temporarily shows `"Compiling..."`, then `"Compiled (<N> ms)"` ~120ms after typing stops.
- [ ] **Zoom controls**:
  - [ ] Click `+` / `Zoom In`: preview pages enlarge.
  - [ ] Click `-` / `Zoom Out`: preview pages shrink.
  - [ ] Click `Fit Width` / `100%`: preview resets to 100%.
  - [ ] Hold Ctrl and scroll mouse wheel: zoom scales smoothly between 40% and 250%.
- [ ] **Resizable split divider**:
  - [ ] Drag divider between editor and preview to resize.
  - [ ] Double-click divider: resets ratio to 50/50.
- [ ] **Compiler diagnostics**:
  - [ ] Introduce a deliberate syntax error (e.g. `#invalid_function_call(`).
  - [ ] Diagnostics banner appears below preview with red error badge.
  - [ ] Click error location badge: editor jumps to error line.
  - [ ] Fix syntax error: error banner clears automatically.

### 6. PDF Export & Standards Dialog
- [ ] **Open Export Dialog**: Click `File > Export PDF...` or `Ctrl+E`.
  - [ ] Export PDF OS window appears.
- [ ] **Version & standard selection**:
  - [ ] Select `PDF 1.7` + `PDF/A-3a`: green checkmark `"✓ Valid Standard Configuration"`.
  - [ ] Select `PDF 1.7` + `PDF/A-1b`: red incompatible warning appears with "Switch to compatible PDF 1.4" button.
  - [ ] Click "Switch to compatible PDF 1.4": version automatically adjusts.
- [ ] **Export to disk**:
  - [ ] Click "Export PDF to Disk...": native OS file save dialog opens.
  - [ ] Choose destination and save: status bar confirms export size and path.

### 7. P2P Collaboration (CRDT + Iroh QUIC)
- [ ] **Host session**:
  - [ ] Open `Project > Collaboration (P2P)...`.
  - [ ] Enter display name and choose cursor color.
  - [ ] Click "Start Hosting Session".
  - [ ] Node ID generated, "Copy Invite Ticket" button enabled.
- [ ] **Join session (second instance or peer)**:
  - [ ] Paste ticket into "Join Existing Session" input and click "Join".
  - [ ] Connected peer list shows peer name with `[Direct (QUIC)]` or `[Relay]` badge.
  - [ ] Edit file on Peer A: Peer B reflects edits in real time with colored cursor and name tag.
  - [ ] Test undo on Peer A: only Peer A's edits are undone, preserving Peer B's edits.

### 8. Settings & Theming
- [ ] **Open Settings**: Click `File > Settings...` or `Ctrl+,`.
- [ ] **Theme toggle**:
  - [ ] Switch to Light Mode: background shifts to `#FFFFFF`, sidebar to `#F4F4F6`.
  - [ ] Switch back to Dark Mode: background shifts to `#1C1B22`.
- [ ] **Editor Font Size**:
  - [ ] Adjust slider from 13.5pt to 18pt: editor text scales instantly.
- [ ] **Word Wrap & Line Numbers**:
  - [ ] Toggle checkboxes: gutter and line wrap update immediately.
- [ ] **Reset Defaults**:
  - [ ] Click "Reset to Defaults": settings revert to baseline defaults.

