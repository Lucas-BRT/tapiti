//! Height: an elevation for every node of the floor (rooms and junctions), from a relief and a steepest slope.
//!
//! The rule is $\lvert h_u - h_v\rvert \le s\,\ell(e)$ on every corridor $e$ between the nodes $u$ and $v$, with $s$ the steepest slope
//! (rise over run) and $\ell(e)$ the length of the corridor. Heights are legal by construction: the layer draws a target for each
//! room and takes the largest legal function below the targets, $h_i = \min_j\,(t_j + s\,d(i,j))$ with $d$ the distance along the
//! corridors, so it is always feasible, whatever the cycles. It needs only sums and comparisons, so it is the same on every platform.
//!
//! Run **after** the layout, the targets are drawn per room and nothing moves. Run **before** it, the layer first makes a [`Terrain`],
//! a ground height that is a pure function of the position, and the generator refuses a corridor whose ends stand at heights the slope
//! cannot join. When the layout is done, the heights of the rooms are the terrain at their centres, lowered where a junction or a
//! corridor the generator did not check makes the rule fail: `HeightField::lowered` says by how much.

use super::{Environment, Layer, LayerPhase, World};
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
    /// Rooms are lower the farther they are from the entrance, down to minus the relief: a floor that goes down. Run before the
    /// layout, the ground falls from one end of the world to the other along its longer side.
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
    /// With the layer before the layout: the most a room was lowered below the terrain to keep the rule. Absent after the layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowered: Option<f64>,
}

/// The ground before the rooms: a height that is a pure function of the position, from integer hashes on a lattice and a polynomial
/// smoothstep, so it draws nothing while the generator runs (it is read in parallel) and is the same on every platform.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    seed: u32,
    style: HeightStyle,
    relief: f64,
    cell: f64,
    world: World,
}

impl Terrain {
    pub fn new(spec: &HeightSpec, seed: u32, cell: f64, world: World) -> Self {
        Self {
            seed,
            style: spec.style,
            relief: spec.relief,
            cell: cell.max(1.0),
            world,
        }
    }

    fn corner(&self, ix: i64, iy: i64) -> f64 {
        let mut z = self.seed
            ^ (ix as u32).wrapping_mul(0x9e37_79b1)
            ^ (iy as u32).wrapping_mul(0x85eb_ca77);
        z = (z ^ (z >> 16)).wrapping_mul(0x85eb_ca6b);
        z = (z ^ (z >> 13)).wrapping_mul(0xc2b2_ae35);
        z ^= z >> 16;
        f64::from(z >> 8) / 16_777_216.0
    }

    /// The ground height at `at`.
    pub fn at(&self, at: V) -> f64 {
        match self.style {
            HeightStyle::Hills => {
                let (fx, fy) = (at[0] / self.cell, at[1] / self.cell);
                let (ix, iy) = (fx.floor(), fy.floor());
                let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
                let (tx, ty) = (smooth(fx - ix), smooth(fy - iy));
                let (ix, iy) = (ix as i64, iy as i64);
                let top = self.corner(ix, iy) * (1.0 - tx) + self.corner(ix + 1, iy) * tx;
                let bottom =
                    self.corner(ix, iy + 1) * (1.0 - tx) + self.corner(ix + 1, iy + 1) * tx;
                self.relief * (top * (1.0 - ty) + bottom * ty)
            }
            HeightStyle::Descent => {
                let w = self.world;
                let (along, span) = if w.x1 - w.x0 >= w.y1 - w.y0 {
                    (at[0] - w.x0, w.x1 - w.x0)
                } else {
                    (at[1] - w.y0, w.y1 - w.y0)
                };
                -self.relief * (along / span.max(1.0)).clamp(0.0, 1.0)
            }
        }
    }
}

/// The length of a corridor: the sum of its segments, in order. The layer, the validator and the measures all use this one.
pub fn corridor_length(pts: &[V]) -> f64 {
    pts.windows(2).map(|s| dist(s[0], s[1])).sum()
}

/// The length of a graph edge, the one every layer uses: a corridor's length; a stair's run, such that a whole storey is within the
/// steepest slope (`storey / maxSlope`), so a stair is always legal for height and spreads humidity and light as the long way it is.
pub fn edge_length(floor: &Floor, e: &crate::generate::Edge) -> f64 {
    if e.stair {
        floor.params.levels.storey / floor.params.environment.height.max_slope.max(1e-9)
    } else {
        corridor_length(&e.pts)
    }
}

/// The level of every node: a room's own, a junction's the level of the rooms its corridors join.
pub fn node_levels(floor: &Floor) -> Vec<usize> {
    let mut level: Vec<Option<usize>> = floor
        .nodes
        .iter()
        .map(|n| match n {
            Node::Room { room } => Some(floor.rooms[*room].level),
            Node::Junction { .. } => None,
        })
        .collect();
    loop {
        let mut changed = false;
        for e in floor.edges.iter().filter(|e| !e.stair) {
            match (level[e.u], level[e.v]) {
                (Some(l), None) => {
                    level[e.v] = Some(l);
                    changed = true;
                }
                (None, Some(l)) => {
                    level[e.u] = Some(l);
                    changed = true;
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    level.into_iter().map(|l| l.unwrap_or(0)).collect()
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
            let length = edge_length(d, e);
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

/// The layer: the settings, and the size of the lattice of its terrain (two cells of the grid).
pub struct HeightLayer(pub HeightSpec, pub f64);

impl HeightLayer {
    fn field(&self, floor: &Floor, rng: &mut Mulberry) -> HeightField {
        let spec = self.0;
        let n = floor.nodes.len();
        let hops = match spec.style {
            HeightStyle::Descent => hops_from_entrance(floor),
            HeightStyle::Hills => vec![0; n],
        };
        let farthest = hops.iter().copied().max().unwrap_or(0).max(1) as f64;
        // A stacked floor stands each level one storey above the last.
        let level = node_levels(floor);
        let storey = floor.params.levels.storey;
        let targets: Vec<f64> = floor
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| match node {
                Node::Room { .. } => {
                    let own = match spec.style {
                        HeightStyle::Hills => spec.relief * rng.next(),
                        HeightStyle::Descent => -spec.relief * hops[i] as f64 / farthest,
                    };
                    // Level 0 adds nothing, not even a zero that would turn a -0.0 into 0.0 in the JSON.
                    if level[i] == 0 {
                        own
                    } else {
                        level[i] as f64 * storey + own
                    }
                }
                Node::Junction { .. } => f64::INFINITY,
            })
            .collect();
        let edges: Vec<(usize, usize, f64)> = floor
            .edges
            .iter()
            .map(|e| (e.u, e.v, edge_length(floor, e)))
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
            lowered: None,
        }
    }

    /// The heights once the layout is done, with the terrain as the targets of the rooms. The rooms the generator joined by checked
    /// corridors stand on the terrain; where a junction or an unchecked corridor breaks the rule the repair lowers a room.
    fn grounded(&self, floor: &Floor, terrain: &Terrain) -> HeightField {
        let targets: Vec<f64> = floor
            .nodes
            .iter()
            .map(|node| match node {
                Node::Room { room } => terrain.at(floor.rooms[*room].center),
                Node::Junction { .. } => f64::INFINITY,
            })
            .collect();
        let edges: Vec<(usize, usize, f64)> = floor
            .edges
            .iter()
            .map(|e| (e.u, e.v, edge_length(floor, e)))
            .collect();
        let nodes = legal_heights(&edges, targets.clone(), self.0.max_slope);
        let lowered = nodes
            .iter()
            .zip(&targets)
            .filter(|(_, t)| t.is_finite())
            .map(|(h, t)| t - h)
            .fold(0.0, f64::max);
        let nodes: Vec<f64> = nodes
            .into_iter()
            .map(|h| if h.is_finite() { h } else { 0.0 })
            .collect();
        let min = nodes.iter().copied().fold(f64::INFINITY, f64::min);
        let max = nodes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        HeightField {
            nodes,
            min: if min.is_finite() { min } else { 0.0 },
            max: if max.is_finite() { max } else { 0.0 },
            lowered: Some(lowered),
        }
    }
}

/// The ground of a Λ and a seed, to draw a continuous field: the terrain of a height layer that runs before the layout, `None`
/// otherwise. After the layout the heights belong to the nodes of the floor and there is no ground between them.
pub fn terrain_of(p: &crate::params::Params, seed: u32, world: World) -> Option<Terrain> {
    let spec = p.environment.height;
    (spec.enabled && spec.phase == LayerPhase::Before).then(|| {
        let mut rng = super::layer_rng(seed, HEIGHT_TAG);
        Terrain::new(
            &spec,
            (rng.next() * 4_294_967_296.0) as u32,
            p.cell_w.max(p.cell_h) * 2.0,
            world,
        )
    })
}

/// The tag of the layer's stream: "HEIG".
pub const HEIGHT_TAG: u32 = 0x4845_4947;

/// Hops over the corridors from the entrance (from the first node when the floor has none); a node nothing reaches is as far as the farthest.
pub fn hops_from_entrance(floor: &Floor) -> Vec<usize> {
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
        HEIGHT_TAG
    }
    fn phase(&self) -> LayerPhase {
        self.0.phase
    }
    fn before(&self, world: World, rng: &mut Mulberry, out: &mut Environment) {
        let seed = (rng.next() * 4_294_967_296.0) as u32;
        out.terrain = Some(Terrain::new(&self.0, seed, self.1, world));
    }
    fn finish(&self, floor: &Floor, out: &mut Environment) {
        if let Some(terrain) = out.terrain.clone() {
            out.height = Some(self.grounded(floor, &terrain));
        }
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        out.height = Some(self.field(floor, rng));
    }
}
