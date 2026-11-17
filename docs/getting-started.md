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

Pick a size (small, medium, large or huge) in the top bar, next to the seed and the Generate button, and a layout (grid, or growth, where rooms grow one from another) in the Layout group on the left. Change the seed and edit any parameter: the floor is generated again a moment after each change. A very large floor can be stopped with the Cancel button (or <kbd>Esc</kbd>), and the floor on the screen stays.

### The three columns

The window has three columns:

- **Parameters**, on the left: the layout, the exploration intent and the environment layers first, then every other group, one bar each. A bar says how many settings the group holds and how many of them differ from the size's defaults. A setting that differs is marked in amber, and its `×` puts the default back. One filter searches every parameter.
- **The map**, in the middle: rulers numbered in grid cells, the pointer in cells and the room under it in the strip below the plan, and a click picks a room (it is outlined in amber and described at the right of the strip). The entrance has a green outline and an IN badge, the exit a violet outline and an OUT badge. Fit shows the floor, not the empty grid around it.
- **Reading**, on the right: the rules V1 to V5, the measures of the floor, the shapes, the doors per room and the phases of the generation. The longer explanation of a line is on hover. A narrow window hides this column; the button at the right of the controls bar (a square with a strip at its edge), or View > Reading panel, shows or hides it.

### Menus and appearance

The menu bar has:

- **File**: open and save a Λ as a JSON file, copy the JSON, copy the floor as SVG, quit.
- **Edit**: reset all to the size.
- **View**: the reading panel.
- **Tools**: the guided experiments and the JSON editor, in a window.
- **Help**: the shortcuts.
- **Settings** (<kbd>Ctrl</kbd>+<kbd>,</kbd>): a window of its own for the options of the application. Each is a choice among presets with a Custom one that takes a value of your own: the theme (System, Light, Dark; the system's is the default); the spacing (Compact, Comfortable, or a row height in pixels); the text size (Small to Larger, or a percentage, for the whole window); the reading panel (Automatic, Shown or Hidden, and Narrow, Normal, Wide or a share of the window); and the motion, timed apart for the interface (a panel sliding, a group opening, a control changing colour) and for the reveal of a new floor: Off, Fast, Normal, Slow or milliseconds. A floor that arrives while the last is still being revealed, as when stepping through seeds, shows at once. They are saved and apply at once.

The rules V1 to V5 and a hash of the seed and Λ (two windows with the same hash show the same floor) are in the Reading column, in the Rules card. The window's title says the size, the seed and whether the floor is valid. The seed is in the controls bar, and the time and the number of draws of the last floor are at the foot.

### Shortcuts and the map

- <kbd>Alt</kbd>+<kbd>1</kbd> to <kbd>4</kbd> or <kbd>Ctrl</kbd>+<kbd>1</kbd> to <kbd>4</kbd> switch the size.
- <kbd>Alt</kbd>+<kbd>R</kbd> draws a random seed; <kbd>Alt</kbd>+<kbd>←</kbd> and <kbd>→</kbd> step the seed; <kbd>Ctrl</kbd>+<kbd>L</kbd> goes to the seed field.
- <kbd>F5</kbd> or <kbd>Ctrl</kbd>+<kbd>Enter</kbd> generates again; <kbd>Esc</kbd> cancels a generation.
- <kbd>Ctrl</kbd>+<kbd>O</kbd> opens a Λ and <kbd>Ctrl</kbd>+<kbd>S</kbd> saves it; <kbd>Ctrl</kbd>+<kbd>C</kbd> copies the JSON and <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> the SVG, outside a text field; <kbd>Ctrl</kbd>+<kbd>N</kbd> puts the parameters back to the size's defaults.
- <kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes the last change (a size, a reset, an experiment, a file or an edit) and <kbd>Ctrl</kbd>+<kbd>Y</kbd> or <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> redoes it. One step is kept each way. The floor of the last change stays in memory, so neither generates it again (Settings > Memory sets which floors are kept, or turns it off).
- <kbd>/</kbd> or <kbd>Ctrl</kbd>+<kbd>F</kbd> goes to the filter. <kbd>Tab</kbd> and <kbd>Shift</kbd>+<kbd>Tab</kbd> move between controls, <kbd>Enter</kbd> opens a group, <kbd>↑</kbd> and <kbd>↓</kbd> change a focused number, <kbd>Esc</kbd> leaves a field.
- <kbd>Ctrl</kbd>+<kbd>B</kbd> shows or hides the Reading column, <kbd>F11</kbd> toggles full screen, <kbd>Ctrl</kbd>+<kbd>+</kbd>, <kbd>Ctrl</kbd>+<kbd>-</kbd> and <kbd>Ctrl</kbd>+<kbd>0</kbd> change the text size, <kbd>Ctrl</kbd>+<kbd>,</kbd> opens Settings, <kbd>F1</kbd> lists the shortcuts, <kbd>Ctrl</kbd>+<kbd>W</kbd> closes the open window and <kbd>Ctrl</kbd>+<kbd>Q</kbd> quits.
- On the map, drag to pan, scroll to zoom, double-click or <kbd>Home</kbd> to fit. With the keyboard, <kbd>Tab</kbd> reaches the map (a cyan ring shows it has the focus): the arrows pan, <kbd>+</kbd> and <kbd>-</kbd> zoom, <kbd>0</kbd> fits, <kbd>[</kbd> and <kbd>]</kbd> pick the previous and the next room (described in the Reading column under Selected room) and <kbd>Esc</kbd> lets go of the pick. The layers you turn on in the Layers menu are kept for the next start. A new seed or size is revealed from the entrance; set the new floor to Off in Settings to show it at once.

## The website and the lab

The website you are reading is a [Topcoat](https://github.com/tokio-rs/topcoat) app. To run it with live reload:

```sh
cargo install topcoat-cli --version 0.12.0 --locked
scripts/build-wasm.sh                      # the WebAssembly module of the lab
topcoat dev -p tapiti-site      # http://127.0.0.1:3000
```

### The lab

The lab (`/lab`) runs the generator in your browser, compiled to WebAssembly, in a worker, so editing never waits for a floor. Its form is built from the same parameter schema as the desktop app, and it reads a floor with the same words: the rules V1 to V5 and a hash of the seed and Λ (the same hash is the same floor, here and in the desktop app), then the measures (single-door rooms, steps to exit, doors per room).

- **Plan.** The entrance has a green outline and an IN chip, the exit a violet outline and an OUT chip. Click a room, or press <kbd>[</kbd> and <kbd>]</kbd> with the plan focused, to pick it: it is outlined in amber and described under Selected room. Rooms can be coloured by shape or by an environment layer the floor has (height, water, humidity, temperature, light), with the scale in the legend; a stacked floor has a level to choose.
- **Keys.** With the plan focused (<kbd>Tab</kbd> reaches it, or the link at the top): the arrows move it, <kbd>+</kbd> and <kbd>-</kbd> zoom, <kbd>0</kbd> or <kbd>Home</kbd> fits it, <kbd>Enter</kbd> picks the room in the middle, <kbd>Esc</kbd> lets go. Anywhere: <kbd>Alt</kbd>+<kbd>1</kbd> to <kbd>4</kbd> pick a size, <kbd>Alt</kbd>+<kbd>R</kbd> a random seed, <kbd>Alt</kbd>+<kbd>←</kbd> and <kbd>→</kbd> the previous or next seed, <kbd>Ctrl</kbd>+<kbd>Z</kbd> and <kbd>Ctrl</kbd>+<kbd>Y</kbd> undo and redo, <kbd>/</kbd> goes to the filter of the parameters and <kbd>?</kbd> lists the keys. Settings can turn the single keys off.
- **Undo and redo** go one step each way, over a size, a reset, an example, a paste or an edit; the floor of the step is kept in memory (Settings says which floors), so neither generates it again.
- **Where you left off.** The last size, seed, changed parameters and overlays are kept in the browser. *Copy link* writes a link that opens the lab on the size, the seed and what differs from the size's defaults; *Paste parameters* takes the JSON that *Copy parameters* writes, or the one the desktop app saves.
- **Parameters.** A parameter you changed is marked in amber, with a button that puts its default back, and each group says how many it holds changed. The ones that change nothing under the present layout or intent are dimmed and say so. A value out of range is not applied, and the message names the field. The reference of every parameter, with its default in each size, is [on its own page](parameters).
- **Settings** (the button in the header): the theme, the text size, the motion, and, for the lab, the reading panel and its width, the floors kept for undo and the single-key shortcuts. Each is a set of presets with a value of your own, kept in the browser; they start from what your system says (its colour scheme, its wish for less motion).

The lab is in English only.

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
