# Installation (/docs/installation)

azo-type targets **Linux** (Wayland & X11), **macOS** (Apple Silicon `aarch64` & Intel `x86_64`), and **Windows** (`x86_64` & `aarch64`).

## Release Binaries [#release-binaries]

Pre-built release archives are published on [GitHub Releases](https://github.com/azocloud/azo-type/releases) (and directly on the [Download page](/download)).

1. Download the archive or installer matching your operating system and CPU architecture.
2. Extract or install the `azo-type` executable (`azo-type.exe` on Windows, `Azo Type.app` on macOS).
3. Launch the binary directly—Typst `0.15.1` and 17 bundled fonts are embedded inside the application, so no external `typst` CLI installation is required.

<Callout type="warn" title="Early Pre-Alpha — No Release Binary Ready Yet">
azo-type is currently in early pre-alpha development, and pre-built release binaries are not yet ready to download. Building from source with Cargo below is currently the recommended way to run the application.
</Callout>

## Building from Source [#building-from-source]

### Prerequisites [#prerequisites]

- **Rust `1.80+`** (stable toolchain via [rustup](https://rustup.rs/))
- **C compiler toolchain** (`gcc`/`clang` on Linux, Xcode Command Line Tools on macOS, MSVC Build Tools on Windows)
- **Linux system libraries:** `fontconfig` development headers plus X11/Wayland libraries

On Debian/Ubuntu Linux, install the required development packages before compiling:

```bash
sudo apt-get update
sudo apt-get install -y libfontconfig1-dev libxkbcommon-dev libwayland-dev libx11-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

### Compile and Run [#compile-and-run]

Clone the repository and build the desktop application from `/app`:

```bash
git clone https://github.com/azocloud/azo-type.git
cd azo-type

# Run debug build
cargo run --manifest-path app/Cargo.toml

# Or build an optimized release binary (LTO fat, stripped)
cargo build --release --manifest-path app/Cargo.toml
```

The release profile in `app/Cargo.toml` uses `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, and `strip = true`. The resulting binary is written to `app/target/release/azo-type` (or `azo-type.exe` on Windows).

## Command-Line Options [#command-line-options]

`azo-type` accepts optional CLI arguments at startup (parsed in `app/src/lib.rs`):

| Invocation | Behavior |
| :--- | :--- |
| `azo-type` | Starts directly in an in-memory blank Typst project ready for immediate editing. |
| `azo-type <folder>` | Opens the specified project directory if it exists on disk. |
| `azo-type --screenshot <path>` | Headless CI capture mode: renders the UI, writes a PNG/JPEG screenshot to `<path>`, and exits. |
| `azo-type --screenshot=<path>` | Equivalent equals-sign form for screenshot output. |