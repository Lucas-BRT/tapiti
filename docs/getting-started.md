# Getting started

## Install

Every [release](https://github.com/Lucas-BRT/tapiti/releases/latest) has the desktop app for each platform. Pick one:

### Linux, any distribution

x86_64, glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+; not Alpine), without `sudo`:

```sh
curl -fsSL https://raw.githubusercontent.com/Lucas-BRT/tapiti/main/install.sh | bash
```

The script downloads the latest release, checks it against the release's `SHA256SUMS` and installs `tapiti` in `~/.local/bin`, with a launcher entry and an icon in `~/.local/share`. It tells you how to put `~/.local/bin` on your `PATH` when that is needed, and never edits your files.

After `bash -s --` it takes:

- `--version X.Y.Z`: install that release instead of the latest.
- `--prefix DIR`: install in `DIR/bin` and `DIR/share` (default `~/.local`).
- `--update`: install the release only if it is not the version already installed.
- `--verify`: also check the build provenance, a GitHub attestation (needs the GitHub CLI 2.49 or newer).
- `--uninstall`: remove Tapiti.

### Debian, Ubuntu, Fedora and openSUSE

A `.deb` for Debian and Ubuntu, an `.rpm` for Fedora and openSUSE:

```sh
sudo apt install ./tapiti_<version>-1_amd64.deb
sudo dnf install ./tapiti-<version>-1.x86_64.rpm
```

Both install `/usr/bin/tapiti`, the launcher entry and the icon. There is no apt or dnf repository yet, so `apt update` does not bring a new release: download the new package and install it the same way, or use `install.sh --update`.

### Windows

Download `tapiti-x86_64-pc-windows-msvc.exe` and run it, or the `.zip` with the same program, the licence and the README. It needs a graphics driver with OpenGL 2. Windows may ask for confirmation, as the program is not signed with a certificate.

### macOS (Apple silicon)

`tapiti-aarch64-apple-darwin.tar.gz` holds `Tapiti.app`; drag it to Applications. It is not signed with a developer certificate, so the first start needs a right-click and Open.

### Verifying a download

Each release also has `SHA256SUMS`, and the files carry a signed build provenance: `gh attestation verify FILE --repo Lucas-BRT/tapiti`.

## Build from source

Tapiti is a Rust workspace. You need a recent stable Rust toolchain ([rustup](https://rustup.rs)); nothing else to try the generator. The desktop app also needs the usual graphics libraries of your platform (on Linux: xkbcommon, Wayland and/or X11, and OpenGL; on Windows a driver with OpenGL 2, which a virtual machine or remote desktop without a GPU may lack).

```sh
git clone https://github.com/Lucas-BRT/tapiti
cd tapiti
```

## The desktop app

```sh
cargo run --release
```

### Generating a floor

Pick a layout (grid, or growth, where rooms grow one from another) and a size (small, medium, large or huge) in the top bar, next to the seed and the Generate button. Change the seed and edit any parameter: the floor is generated again a moment after each change. A very large floor can be stopped with the Cancel button (or <kbd>Esc</kbd>), and the floor on the screen stays.

### The three columns

The window has three columns:

- **Parameters**, on the left: the layout, the exploration intent and the environment layers first, then every other group, one bar each. A bar says how many settings the group holds and how many of them differ from the size's defaults. A setting that differs is marked in amber, and its `×` puts the default back. One filter searches every parameter.
- **The map**, in the middle: rulers numbered in grid cells, the pointer in cells and the room under it in the strip below the plan, and a click picks a room.
- **Reading**, on the right: the rules V1 to V5, the measures of the floor, the shapes, the doors per room and the phases of the generation. The longer explanation of a line is on hover. A narrow window hides this column; the button at the right of the controls bar (a square with a strip at its edge), or View > Reading panel, shows or hides it.

### The command line

At the foot of the window: `seed N`, `size small|medium|large|huge` (`s`, `m`, `l` and `h` also work), `step ±N`, `random`, `reset`, `layer NAME`, `filter TEXT` and `group NAME`. <kbd>:</kbd> or <kbd>Ctrl</kbd>+<kbd>K</kbd> goes to it.

### Menus and appearance

The menu bar has:

- **File**: open and save a Λ as a JSON file, copy the JSON, copy the floor as SVG, quit.
- **Edit**: reset all to the size.
- **View**: the reading panel.
- **Tools**: the guided experiments and the JSON editor, in a window.
- **Help**: the shortcuts.
- **Settings** (<kbd>Ctrl</kbd>+<kbd>,</kbd>): a window of its own for the options of the application. Each is a choice among presets with a Custom one that takes a value of your own: the theme (System, Light, Dark; the system's is the default); the spacing (Compact, Comfortable, or a row height in pixels); the text size (Small to Larger, or a percentage, for the whole window); the reading panel (Automatic, Shown or Hidden, and Narrow, Normal, Wide or a share of the window); and the motion, timed apart for the interface (a panel sliding, a group opening, a control changing colour) and for the reveal of a new floor: Off, Fast, Normal, Slow or milliseconds. A floor that arrives while the last is still being revealed, as when stepping through seeds, shows at once. They are saved and apply at once.

On the right of the same bar are a hash of the seed and Λ (two windows with the same hash show the same floor) and V1 to V5. The seed is in the controls bar, and the time and the number of draws of the last floor are at the foot.

### Shortcuts and the map

- <kbd>Alt</kbd>+<kbd>1</kbd> to <kbd>4</kbd> switch the size.
- <kbd>Alt</kbd>+<kbd>R</kbd> draws a random seed.
- <kbd>Alt</kbd>+<kbd>←</kbd> and <kbd>→</kbd> step the seed.
- <kbd>/</kbd> goes to the filter; <kbd>Tab</kbd> and <kbd>Shift</kbd>+<kbd>Tab</kbd> move between controls, <kbd>Enter</kbd> opens a group, <kbd>↑</kbd> and <kbd>↓</kbd> change a focused number, <kbd>Esc</kbd> leaves a field.
- On the map, drag to pan, scroll to zoom, double-click to fit. A new seed or size is revealed from the entrance; set the new floor to Off in Settings to show it at once.

## The website and the lab

The website you are reading is a [Topcoat](https://github.com/tokio-rs/topcoat) app. To run it with live reload:

```sh
cargo install topcoat-cli
scripts/build-wasm.sh                      # the WebAssembly module of the lab
topcoat dev -p tapiti-site      # http://127.0.0.1:3000
```

The lab (`/lab`) runs the generator in your browser, compiled to WebAssembly. Its form is built from the same parameter schema as the desktop app. The lab is in English only.

## Docker

```sh
docker compose -f compose.local.yml up --build
```

serves the same site on <http://localhost:8090>. `compose.yml` is the version meant for a Dokploy deployment; set `TAPITI_SITE_URL` to the public address so canonical links and social previews point to it.

## Using the crate

`tapiti-core` is the generator, with no UI. Add it as a git dependency:

```toml
[dependencies]
tapiti-core = { git = "https://github.com/Lucas-BRT/tapiti" }
```

```rust
use tapiti_core::{
    params::{params_of_size, Size},
    stage::stage,
};

fn main() {
    let params = params_of_size(Size::Medium);
    // Up to 8 draws: a floor the validator rejects is generated again from a derived seed.
    let staged = stage(42, &params, 8);
    assert!(staged.violations.is_empty());
    println!("{} rooms, {} corridors", staged.d.rooms.len(), staged.d.edges.len());
}
```

The same seed and parameters always give the same floor, native or WebAssembly.

## Command-line tools

```sh
cargo run --release -p tapiti-core --example dump               # a hash per stage, to compare two builds
cargo run -p tapiti-render --example export_svg -- medium 26 floor.svg dark   # size, seed, output, theme
cargo bench -p tapiti-core --bench generator                    # Criterion benchmarks
```

## Where to go next

- [The mathematical model](model): what a floor is, how it is validated and measured.
- [Parameters](parameters): every parameter of Λ, with its range.
- [Benchmarks](performance): how to measure the generator.
- [Roadmap](roadmap): what is unfinished.
