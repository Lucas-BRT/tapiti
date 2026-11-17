//! The Tapiti model (`docs/model.md`) as a Rust crate compiled to WebAssembly: rooms of any shape,
//! continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator and the graph's measures.
//!
//! The browser calls [`stage_json`]: the stage, its violations and its measures come back as one JSON string, so the page only draws.

use tapiti_core::{clock, generate, metrics, par, params, stage};
use tapiti_render::{readout, relevance, view};

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Answer<'a> {
    floor: &'a generate::Floor,
    violations: &'a [String],
    draws: u32,
    metrics: metrics::Metrics,
    /// Milliseconds the generation took inside the module (when the host gave a clock), for the page's panel.
    ms: f64,
    /// The measures, as the desktop app and the landing page read them (one id, label, value, share and explanation each).
    readout: Vec<ReadoutRow>,
    /// The five rules: code, name, what each checks.
    rules: Vec<[&'static str; 3]>,
    /// A hash of the seed and the Λ of the floor: the same hash is the same floor, in the browser and in the desktop app.
    hash: String,
    /// The environment layers the floor carries, each with a colour per room (by room id) and its scale.
    tints: Vec<Tint>,
}

#[derive(Serialize)]
struct ReadoutRow {
    id: &'static str,
    label: &'static str,
    value: String,
    share: Option<f32>,
    tip: String,
}

#[derive(Serialize)]
struct Tint {
    id: &'static str,
    label: &'static str,
    unit: &'static str,
    /// The ends of the scale, as the inspector prints them.
    lo: String,
    hi: String,
    /// The colour of each room as `#rrggbb`, or `null` for a dead room.
    colors: Vec<Option<String>>,
    /// The two ends of the colour ramp, for the legend.
    ramp: [String; 2],
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn tints_of(floor: &generate::Floor) -> Vec<Tint> {
    view::Channel::ALL
        .into_iter()
        .filter(|c| c.available(floor))
        .filter_map(|c| {
            let scale = view::scale(floor, c)?;
            let colors = view::room_values(floor, c)
                .into_iter()
                .map(|v| v.map(|v| hex(view::color(c, view::position(scale, v)))))
                .collect();
            Some(Tint {
                id: c.id(),
                label: c.label(),
                unit: c.unit(),
                lo: view::format_value(c, scale.0),
                hi: view::format_value(c, scale.1),
                colors,
                ramp: [hex(view::color(c, 0.0)), hex(view::color(c, 1.0))],
            })
        })
        .collect()
}

/// Λ of a size (1 small, 2 medium, 3 large, 4 huge) as JSON.
#[wasm_bindgen]
pub fn default_params(size: u32) -> String {
    serde_json::to_string(&params::params_of_size(params::Size::of_index(
        size as usize,
    )))
    .unwrap()
}

/// Λ of a size with every group present, as JSON: `default_params` leaves out the zones, the levels and the environment while they are
/// off (a floor does not need them), but a form needs their fields, at their defaults, to edit them.
#[wasm_bindgen]
pub fn full_params(size: u32) -> String {
    readout::full_params(size as usize).to_string()
}

/// Λ of a size with every shape, junction and connection option on (the landing page's showcase), as JSON.
#[wasm_bindgen]
pub fn rich_params(size: u32) -> String {
    serde_json::to_string(&params::rich_of_size(params::Size::of_index(size as usize))).unwrap()
}

/// A valid stage for `seed` and the Λ in `params_json`, as JSON: `{ floor, violations, draws, metrics, ms }`.
#[wasm_bindgen]
pub fn stage_json(seed: u32, params_json: &str) -> Result<String, JsError> {
    let p: params::Params =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let t0 = clock::now_ms();
    let s = stage::stage(seed, &p, 8);
    let ms = clock::now_ms() - t0;
    let m = metrics::metrics_of(&s.d);
    serde_json::to_string(&Answer {
        floor: &s.d,
        violations: &s.violations,
        draws: s.draws,
        readout: readout::rows(&m)
            .into_iter()
            .map(|r| ReadoutRow {
                id: r.id,
                label: r.label,
                value: r.value,
                share: r.share,
                tip: r.tip,
            })
            .collect(),
        rules: readout::RULES
            .iter()
            .map(|(a, b, c)| [*a, *b, *c])
            .collect(),
        hash: readout::floor_hash(seed, &s.d.params),
        tints: tints_of(&s.d),
        metrics: m,
        ms,
    })
    .map_err(|e| JsError::new(&e.to_string()))
}

/// Of the parameter paths in `paths_json` (a JSON array of strings), those that cannot change the floor under the Λ in `params_json`:
/// the form greys them out. The same rules as the desktop app's.
#[wasm_bindgen]
pub fn inapplicable(params_json: &str, paths_json: &str) -> Result<String, JsError> {
    let p: params::Params =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let paths: Vec<String> = serde_json::from_str(paths_json)
        .map_err(|e| JsError::new(&format!("bad list of paths: {e}")))?;
    let out: Vec<&String> = paths
        .iter()
        .filter(|path| !relevance::applies(path, &p))
        .collect();
    serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))
}

/// The ground of a height layer that runs before the layout, at the point `(x, y)`, for the seed and the Λ in `params_json`: to draw a
/// continuous field. `NaN` when the Λ has no such layer; after the layout the heights belong to the nodes of the floor.
#[wasm_bindgen]
pub fn ground_at(seed: u32, params_json: &str, x: f64, y: f64) -> Result<f64, JsError> {
    let p: params::Params =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let world = tapiti_core::environment::World {
        x0: 0.0,
        y0: 0.0,
        x1: p.cols.max(1) as f64 * p.cell_w,
        y1: p.rows.max(1) as f64 * p.cell_h,
    };
    Ok(
        tapiti_core::environment::height::terrain_of(&p, seed, world)
            .map_or(f64::NAN, |t| t.at([x, y])),
    )
}

/// The fingerprint of how the floor of `seed` and the Λ in `params_json` looks in 3D (the hash of its mesh and of two pictures of it), as
/// three hexadecimal numbers: to check that this module draws what the native build draws.
#[wasm_bindgen]
pub fn view_digest(seed: u32, params_json: &str) -> Result<String, JsError> {
    let p: params::Params =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let [mesh, outside, inside] = tapiti_render::digest::of(&generate::generate(seed, &p));
    Ok(format!("{mesh:016x} {outside:016x} {inside:016x}"))
}

/// How many threads the module computes with (1 for the single-threaded build).
#[wasm_bindgen]
pub fn threads() -> u32 {
    par::threads() as u32
}

/// Times `count` stages of one size from the seeds `from..from + count`, one after another, and returns `{ invalid, ms, threads }`.
/// The page's worker pool gives each worker a slice of the seeds (see `apps/site/assets/lab/lab.js`); natively the stages of a slice also split over rayon's threads.
#[wasm_bindgen]
pub fn bench(size: u32, from: u32, count: u32) -> String {
    let p = params::params_of_size(params::Size::of_index(size as usize));
    let seeds: Vec<u32> = (from..from + count).collect();
    let t0 = clock::now_ms();
    let invalid: usize = par::pmap(&seeds, |seed| {
        usize::from(!stage::stage(*seed, &p, 8).violations.is_empty())
    })
    .iter()
    .sum();
    format!(
        "{{\"invalid\":{},\"ms\":{},\"threads\":{}}}",
        invalid,
        clock::now_ms() - t0,
        par::threads()
    )
}
