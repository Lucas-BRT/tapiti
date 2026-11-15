//! Height: an elevation for every node of the floor (rooms and junctions), from a relief and a steepest slope.
//!
//! The rule is $\lvert h_u - h_v\rvert \le s\,\ell(e)$ on every corridor $e$ between the nodes $u$ and $v$, with $s$ the steepest slope
//! (rise over run) and $\ell(e)$ the length of the corridor. Heights are legal by construction: the layer draws a target for each
//! room and takes the largest legal function below the targets, $h_i = \min_j\,(t_j + s\,d(i,j))$ with $d$ the distance along the
//! corridors, so it is always feasible, whatever the cycles. It needs only sums and comparisons, so it is the same on every platform.

use super::{Environment, Layer, LayerPhase};
use crate::{
    generate::{Floor, Node},
    geom::{dist, V},
    rng::{Mulberry, Rand},
};
use serde::{Deserialize, Serialize};

/// How the targets are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeightStyle {
    /// Each room draws a height between 0 and the relief: hills and hollows.
    #[default]
    Hills,
    /// Rooms are lower the farther they are from the entrance, down to minus the relief: a floor that goes down.
    Descent,
}

/// The settings of the layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HeightSpec {
    pub enabled: bool,
    pub phase: LayerPhase,
    /// The steepest slope of a corridor, rise over run.
    pub max_slope: f64,
    /// The span of the targets, in the units of the floor.
    pub relief: f64,
    pub style: HeightStyle,
}

impl Default for HeightSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            phase: LayerPhase::After,
            max_slope: 0.25,
            relief: 160.0,
            style: HeightStyle::Hills,
        }
    }
}

impl HeightSpec {
    /// What is wrong with the settings, if anything.
    pub fn error(&self) -> Option<&'static str> {
        if !(self.max_slope.is_finite() && self.max_slope > 0.0) {
            return Some("Height: the steepest slope must be greater than 0.");
        }
        if !(self.relief.is_finite() && self.relief >= 0.0) {
            return Some("Height: the relief cannot be negative.");
        }
        None
    }
}

/// The elevation of every node of the floor, in the order of `Floor::nodes`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightField {
    pub nodes: Vec<f64>,
    pub min: f64,
    pub max: f64,
}

/// The length of a corridor: the sum of its segments, in order. The layer, the validator and the measures all use this one.
pub fn corridor_length(pts: &[V]) -> f64 {
    pts.windows(2).map(|s| dist(s[0], s[1])).sum()
}

/// The largest function below `targets` that changes by at most `slope * length` along every edge `(u, v, length)`.
/// A target of infinity means no preference. Bellman-Ford sweeps in the order of `edges`: sums and comparisons only.
pub fn legal_heights(edges: &[(usize, usize, f64)], mut h: Vec<f64>, slope: f64) -> Vec<f64> {
    for _ in 0..=h.len() {
        let mut changed = false;
        for &(u, v, length) in edges {
            let step = slope * length;
            if h[u] + step < h[v] {
                h[v] = h[u] + step;
                changed = true;
            }
            if h[v] + step < h[u] {
                h[u] = h[v] + step;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    h
}

/// The slope `|h_u - h_v| / length` of every corridor of the floor, in the order of `Floor::edges` (0 for a corridor with no length).
pub fn edge_slopes(d: &Floor, field: &HeightField) -> Vec<f64> {
    d.edges
        .iter()
        .map(|e| {
            let length = corridor_length(&e.pts);
            let rise = (field.nodes[e.u] - field.nodes[e.v]).abs();
            if length > 0.0 {
                rise / length
            } else {
                0.0
            }
        })
        .collect()
}

/// The validator's check of the height: one message per corridor steeper than the Λ allows.
pub fn check(d: &Floor, field: &HeightField) -> Vec<String> {
    let spec = d.params.environment.height;
    let mut out = Vec::new();
    if field.nodes.len() != d.nodes.len() {
        out.push("E1 height: there is not one height for each node".to_string());
        return out;
    }
    for (e, slope) in d.edges.iter().zip(edge_slopes(d, field)) {
        // The largest legal function leaves some corridors exactly at the limit: a hair of room for the last digit.
        if slope > spec.max_slope * (1.0 + 1e-9) + 1e-12 {
            out.push(format!(
                "E1 height: corridor {} is {slope:.3} steep (at most {})",
                e.id, spec.max_slope
            ));
        }
    }
    out
}

/// The layer: it runs after the layout and only reads the floor.
pub struct HeightLayer(pub HeightSpec);

impl HeightLayer {
    fn field(&self, floor: &Floor, rng: &mut Mulberry) -> HeightField {
        let spec = self.0;
        let n = floor.nodes.len();
        let hops = match spec.style {
            HeightStyle::Descent => hops_from_entrance(floor),
            HeightStyle::Hills => vec![0; n],
        };
        let farthest = hops.iter().copied().max().unwrap_or(0).max(1) as f64;
        let targets: Vec<f64> = floor
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| match node {
                Node::Room { .. } => match spec.style {
                    HeightStyle::Hills => spec.relief * rng.next(),
                    HeightStyle::Descent => -spec.relief * hops[i] as f64 / farthest,
                },
                Node::Junction { .. } => f64::INFINITY,
            })
            .collect();
        let edges: Vec<(usize, usize, f64)> = floor
            .edges
            .iter()
            .map(|e| (e.u, e.v, corridor_length(&e.pts)))
            .collect();
        let mut nodes = legal_heights(&edges, targets, spec.max_slope);
        for h in &mut nodes {
            if !h.is_finite() {
                *h = 0.0;
            }
        }
        let min = nodes.iter().copied().fold(f64::INFINITY, f64::min);
        let max = nodes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        HeightField {
            nodes,
            min: if min.is_finite() { min } else { 0.0 },
            max: if max.is_finite() { max } else { 0.0 },
        }
    }
}

/// Hops over the corridors from the entrance (from the first node when the floor has none); a node nothing reaches is as far as the farthest.
fn hops_from_entrance(floor: &Floor) -> Vec<usize> {
    let n = floor.nodes.len();
    let start = floor
        .entrance
        .and_then(|r| {
            floor
                .nodes
                .iter()
                .position(|node| matches!(node, Node::Room { room } if *room == r))
        })
        .unwrap_or(0);
    let mut hops = vec![usize::MAX; n];
    if n == 0 {
        return hops;
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &floor.edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    hops[start] = 0;
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if hops[v] == usize::MAX {
                hops[v] = hops[u] + 1;
                queue.push_back(v);
            }
        }
    }
    let farthest = hops
        .iter()
        .copied()
        .filter(|h| *h != usize::MAX)
        .max()
        .unwrap_or(0);
    hops.iter()
        .map(|h| if *h == usize::MAX { farthest } else { *h })
        .collect()
}

impl Layer for HeightLayer {
    fn name(&self) -> &'static str {
        "height"
    }
    fn tag(&self) -> u32 {
        // "HEIG"
        0x4845_4947
    }
    fn phase(&self) -> LayerPhase {
        LayerPhase::After
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        out.height = Some(self.field(floor, rng));
    }
}
