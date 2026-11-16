# Development

How to run, build and check Tapiti, and where things are in the repository. To install a release instead, see [getting started](getting-started#install).

## Run

```bash
cargo run                           # opens the native desktop app (egui) (workspace default; fast dev build)
cargo run --release                 # optimized desktop build
scripts/build-wasm.sh && topcoat dev -p tapiti-site     # the website (landing, docs, lab) on http://127.0.0.1:3000/, rebuilt on every change (`cargo install topcoat-cli`)
docker compose -f compose.local.yml up --build   # the same site as a container on http://localhost:8090/
node scripts/lab-smoke.mjs                   # smoke test of the lab in headless Firefox (site running)
node scripts/site-smoke.mjs                  # the landing page, the 404, the docs (site running; AXE_JS=… adds axe-core)
node scripts/site-shots.mjs shots            # screenshots of the pages at desktop and phone width
scripts/build-wasm.sh                          # only the module: cargo build --release --target wasm32-unknown-unknown + wasm-bindgen → apps/site/pkg
cargo test --release -p tapiti-core   # validity of the 5 levels, determinism, density; add `profile`, `scale`, `bench -- --nocapture` for timings
cargo test --release -p tapiti-core --no-default-features   # the same, single-threaded
cargo bench --release -p tapiti-core --bench generator  # Criterion benchmarks
cargo run --release -p tapiti-core --example dump  # a hash per stage, to compare two builds
scripts/build-wasm.sh simd && node scripts/bench-wasm.mjs 5 100      # SIMD vs not, in Node
```

## Workspace

The code is split into workspace packages so frontend changes do not require rebuilding the generator, WASM bindings, web server, and desktop app together.
Cargo keeps one shared `target/` cache: `cargo run` builds only the default desktop app and its core dependency; `scripts/build-wasm.sh` builds the WASM package and core; the site is compiled only when selected. The desktop (egui/eframe, glow renderer) enables only the Wayland and X11 backends. For everyday UI work, use the incremental development profile with plain `cargo run`; release builds retain optimization and thin LTO for performance measurements.

The root virtual workspace sets only `apps/desktop` as its default member, so plain `cargo run` still opens the desktop app. To build a package directly, use `cargo build -p tapiti-core`, `cargo build -p tapiti-wasm --target wasm32-unknown-unknown`, or `cargo build -p tapiti-site`. Cargo compiles the selected package and its dependency graph, rather than every workspace member. Development builds omit debug symbols and optimize only the generator core; release builds use thin LTO and multiple codegen units to shorten link times while retaining native optimization.

## The website

The website is a [Topcoat](https://github.com/tokio-rs/topcoat) app (`apps/site`): server-rendered pages in English, with the browser lab at `/lab` (a Topcoat page; its script, `apps/site/assets/lab/`, drives the WebAssembly module served from `/lab/pkg/`). `HOST` and `PORT` set the address; `compose.yml` is the Dokploy deployment.

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `crates/tapiti-wasm/Cargo.toml` (`=0.2.130`).

## Determinism

The same seed and Λ always draw the same stage (the rng is Mulberry32). Every optimization was checked against that: `cargo run --release -p tapiti-core --example dump`
hashes 150 stages (the four sizes × 30 seeds, then 30 grown floors) and the hashes did not move, single-threaded or parallel.

## Layout of the repository

### The generator

| file | what |
|---|---|
| `crates/tapiti-core/src/geom.rs` | directions Θ₈, shapes, distances (exact and with a limit), corridor bands and joints |
| `crates/tapiti-core/src/params.rs` | Λ (serde, camelCase JSON), the four sizes, the layouts and the topology intents with their guides |
| `crates/tapiti-core/src/generate/` | the generator, valid by construction, one file per step: scaffold, rooms and shapes, doors, candidates and corridors, tree, cycles with T/X, leaf chambers, entrance and exit; the layouts (growth, binary partition), the intents that plan rooms (spine, ring), zones and stacked levels |
| `crates/tapiti-core/src/environment/` | the layers over a floor (height, water, climate, light, events) and where things are placed on it |
| `crates/tapiti-core/src/validate.rs` | V1 to V5 from the stage alone, the intent's requirements, the environment checks (E1 to E5) and the stairs (V7) |
| `crates/tapiti-core/src/metrics.rs` | β, δ, bridges, articulation rooms, Menger routes, diameter, λ₂, and the measures of each layer |
| `crates/tapiti-core/src/topology.rs` | the graph operations the topology intents use |
| `crates/tapiti-core/src/layout.rs` | how regular a floor's room layout looks, to compare a grid with a grown layout |
| `crates/tapiti-core/src/spatial.rs` | a uniform-grid broad phase for the distance checks |
| `crates/tapiti-core/src/stage.rs` | a valid stage: redraws from a derived seed when the validator objects |
| `crates/tapiti-core/src/par.rs` | the one place that decides parallel or sequential (rayon, or plain loops) |
| `crates/tapiti-core/src/clock.rs` | `performance.now()` in the browser, the system clock natively |
| `crates/tapiti-core/src/rng.rs`, `detmath.rs` | Mulberry32, and `sin`, `cos`, `exp`, `ln` from one pure-Rust implementation, so a seed draws the same stage everywhere |
| `crates/tapiti-core/src/cancel.rs` | a token to stop a generation that takes too long |
| `crates/tapiti-core/tests/`, `benches/`, `examples/` | the cancellation tests, Criterion benchmarks, deterministic dumps, profiling and layout studies (the model tests are unit tests in `src/`) |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.

### Applications and support crates

| file | what |
|---|---|
| `apps/desktop/` | native desktop application (egui); selected by bare `cargo run` |
| `apps/site/` | the website: landing page, docs and the lab (Topcoat; `topcoat dev -p tapiti-site`) |
| `crates/tapiti-render/` | SVG rendering of floors for the site, docs and social images |
| `crates/tapiti-schema/` | the parameters of Λ as data (groups, ranges, help), shared by the desktop app and the docs |
| `docs/` | the documentation (Markdown, math as `$…$`), rendered by the site and readable on GitHub |
| `crates/tapiti-wasm/` | the WASM API: `default_params(size)`, `rich_params(size)`, `stage_json(seed, params)`, `ground_at(seed, params, x, y)`, `view_digest(seed, params)`, `bench(size, from, count)`, `threads()` |
