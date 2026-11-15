//! Light: an intensity from 0 to 1 at every node, from the sun at the openings to the surface and from direct sources placed on the floor.
//!
//! Every source has a power. A node passes on to a neighbour the fraction $\tau_e = \lambda\,\max(0,\,1-\ell(e)/r)$ of what it has, with
//! $\lambda$ the loss at a door, $\ell(e)$ the length of the corridor and $r$ the range; the light at a node is the strongest that
//! reaches it, $I_v=\max(P_v,\max_u I_u\tau_{uv})$. Every $\tau_e<1$, so cycles never add light and the sweeps in the order of the
//! corridors converge. The sun is the daylight times a curve of the hour of the day, $\max\bigl(0,\,1-((t-12)/6)^2\bigr)$, which is
//! $1$ at noon and $0$ from 18 to 6. Only sums, products and quotients: the same on every platform.

use super::{
    height::edge_length,
    placement::{place, Placement},
    Environment, Layer, LayerPhase,
};
use crate::{
    generate::{Floor, Node},
    rng::Mulberry,
};
use serde::{Deserialize, Serialize};

/// The settings of the layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LightSpec {
    pub enabled: bool,
    /// The hour of the day, from 0 to 24.
    pub time: f64,
    /// The power of the sun at noon, from 0 to 1.
    pub daylight: f64,
    /// Rooms that open to the surface besides the entrance.
    pub openings: usize,
    /// Direct sources (torches, lamps, glowing things) placed on the floor.
    pub torches: usize,
    /// The power of a direct source, from 0 to 1.
    pub torch_power: f64,
    /// Where the direct sources go.
    pub placement: Placement,
    /// The length of corridor over which light fades to nothing.
    pub range: f64,
    /// The fraction of light that gets through a door, from 0 to 1.
    pub door_loss: f64,
}

impl Default for LightSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            time: 12.0,
            daylight: 1.0,
            openings: 1,
            torches: 0,
            torch_power: 0.8,
            placement: Placement::Uniform,
            range: 400.0,
            door_loss: 0.85,
        }
    }
}

impl LightSpec {
    /// What is wrong with the settings, if anything.
    pub fn error(&self) -> Option<&'static str> {
        if !(self.time.is_finite() && (0.0..=24.0).contains(&self.time)) {
            return Some("Light: the hour goes from 0 to 24.");
        }
        for (v, what) in [
            (self.daylight, "daylight"),
            (self.torch_power, "torch power"),
            (self.door_loss, "door transmission"),
        ] {
            if !(v.is_finite() && (0.0..=1.0).contains(&v)) {
                return Some(match what {
                    "daylight" => "Light: the daylight goes from 0 to 1.",
                    "torch power" => "Light: the torch power goes from 0 to 1.",
                    _ => "Light: the door transmission goes from 0 to 1.",
                });
            }
        }
        if self.openings > 64 || self.torches > 256 {
            return Some("Light: at most 64 openings and 256 direct sources.");
        }
        if !(self.range.is_finite() && self.range > 0.0) {
            return Some("Light: the range must be greater than 0.");
        }
        None
    }

    /// The sun at the hour of the day, from 0 to the daylight.
    pub fn sun(&self) -> f64 {
        let x = (self.time - 12.0) / 6.0;
        self.daylight * (1.0 - x * x).max(0.0)
    }
}

/// The light of every node, in the order of `Floor::nodes`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightField {
    pub intensity: Vec<f64>,
    /// The nodes that open to the surface: the entrance first, when the floor has one.
    pub openings: Vec<usize>,
    /// The nodes with a direct source.
    pub torches: Vec<usize>,
    /// What each node gives on its own: the sun at an opening, the power of a source, else 0.
    pub power: Vec<f64>,
}

/// What a corridor of length `length` lets through.
pub fn transmission(spec: &LightSpec, length: f64) -> f64 {
    spec.door_loss * (1.0 - length / spec.range).max(0.0)
}

/// The strongest light that reaches each node from the sources `power`, by sweeps in the order of the corridors.
pub fn spread(floor: &Floor, spec: &LightSpec, power: &[f64]) -> Vec<f64> {
    let mut i = power.to_vec();
    let edges: Vec<(usize, usize, f64)> = floor
        .edges
        .iter()
        .map(|e| (e.u, e.v, transmission(spec, edge_length(floor, e))))
        .collect();
    for _ in 0..=i.len() {
        let mut changed = false;
        for &(u, v, t) in &edges {
            if i[u] * t > i[v] {
                i[v] = i[u] * t;
                changed = true;
            }
            if i[v] * t > i[u] {
                i[u] = i[v] * t;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    i
}

/// The layer: it runs after the layout, and places its sources with the height when that layer is on.
pub struct LightLayer(pub LightSpec);

impl Layer for LightLayer {
    fn name(&self) -> &'static str {
        "light"
    }
    fn tag(&self) -> u32 {
        // "LGHT"
        0x4c47_4854
    }
    fn phase(&self) -> LayerPhase {
        LayerPhase::After
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        let spec = self.0;
        let n = floor.nodes.len();
        let heights = out.height.as_ref().map(|h| h.nodes.as_slice());
        let mut openings: Vec<usize> = floor
            .entrance
            .and_then(|r| {
                floor
                    .nodes
                    .iter()
                    .position(|node| matches!(node, Node::Room { room } if *room == r))
            })
            .into_iter()
            .collect();
        let extra = place(
            floor,
            heights,
            Placement::HighGround,
            spec.openings,
            &openings,
            rng,
        );
        openings.extend(extra);
        let torches = place(floor, heights, spec.placement, spec.torches, &[], rng);
        let mut power = vec![0.0_f64; n];
        for &i in &openings {
            power[i] = power[i].max(spec.sun());
        }
        for &i in &torches {
            power[i] = power[i].max(spec.torch_power);
        }
        out.light = Some(LightField {
            intensity: spread(floor, &spec, &power),
            openings,
            torches,
            power,
        });
    }
}

/// The validator's check of the light (E4): every intensity lies from 0 to 1, no node is dimmer than what a neighbour lets through
/// or than what it gives itself, and every source gives what its settings say.
pub fn check(d: &Floor, field: &LightField) -> Vec<String> {
    let spec = d.params.environment.light;
    let n = d.nodes.len();
    let mut out = Vec::new();
    if field.intensity.len() != n || field.power.len() != n {
        out.push("E4 light: there is not one intensity for each node".to_string());
        return out;
    }
    let eps = 1e-9;
    for (i, (v, p)) in field.intensity.iter().zip(&field.power).enumerate() {
        if !(-eps..=1.0 + eps).contains(v) {
            out.push(format!("E4 light: the intensity at node {i} is {v}"));
        }
        if *v < p - eps {
            out.push(format!(
                "E4 light: node {i} is dimmer than the source it holds"
            ));
        }
    }
    for e in &d.edges {
        let t = transmission(&spec, edge_length(d, e));
        for (a, b) in [(e.u, e.v), (e.v, e.u)] {
            if field.intensity[b] < field.intensity[a] * t - eps {
                out.push(format!(
                    "E4 light: node {b} is dimmer than node {a} lets through"
                ));
            }
        }
    }
    for &i in &field.torches {
        if (field.power[i] - field.power[i].max(spec.torch_power)).abs() > eps {
            out.push(format!(
                "E4 light: the source at node {i} gives less than its power"
            ));
        }
    }
    out
}
