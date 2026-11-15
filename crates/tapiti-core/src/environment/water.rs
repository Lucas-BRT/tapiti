//! Water and humidity, on top of the height layer: water runs downhill along the corridors and pools where it cannot go lower.
//!
//! Every node of the floor (rooms and junctions) drains to its lowest strictly lower neighbour (ties to the lowest index); a node with
//! none is a sink, and each sink owns the nodes that drain to it, its basin. A few rooms, drawn from a stream of their own and
//! weighted toward high ground, pour a volume of water into their basin. A basin holds water up to its lowest saddle, the lowest
//! $\max(h_u,h_v)$ over its boundary corridors, at a level found from the nodes sorted by height with sums and one division. What it
//! cannot hold crosses the saddle into the next basin; if that one is already full at the same saddle the two merge and one level is
//! solved for both. Nothing is lost: the volume held is the volume poured.
//!
//! Humidity is a number from 0 to 1 per node that falls with the distance along the corridors to the nearest wet node, down to 0 at
//! `reach`; a wet node is 1. It comes from the water only: the depth of the floor plays no part.

use super::{Environment, Layer, LayerPhase};
use crate::{
    generate::{Floor, Node},
    rng::{Mulberry, Rand},
};
use serde::{Deserialize, Serialize};

/// The settings of the layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WaterSpec {
    pub enabled: bool,
    /// How many rooms pour water.
    pub sources: usize,
    /// What each source pours, as a depth of water over an average room.
    pub flow: f64,
    /// The distance along the corridors at which humidity falls to 0.
    pub reach: f64,
}

impl Default for WaterSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            sources: 2,
            flow: 30.0,
            reach: 600.0,
        }
    }
}

impl WaterSpec {
    /// What is wrong with the settings, if anything.
    pub fn error(&self) -> Option<&'static str> {
        if self.sources == 0 || self.sources > 64 {
            return Some("Water: from 1 to 64 sources.");
        }
        if !(self.flow.is_finite() && self.flow >= 0.0) {
            return Some("Water: the flow cannot be negative.");
        }
        if !(self.reach.is_finite() && self.reach > 0.0) {
            return Some("Water: the humidity reach must be greater than 0.");
        }
        None
    }
}

/// What the water did to the floor, per node in the order of `Floor::nodes`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterField {
    /// The surface of the water at each node: the height of the floor where it is dry.
    pub level: Vec<f64>,
    /// 0 (dry, farther than `reach` from any water) to 1 (under water).
    pub humidity: Vec<f64>,
    /// The nodes that pour water.
    pub sources: Vec<usize>,
    /// The way each source's water runs down to the sink it ends in.
    pub routes: Vec<Vec<usize>>,
    /// The water poured in all, in the units of the floor cubed.
    pub volume: f64,
}

/// The floor area of each node: a room's area, a junction's corridor width squared.
pub fn node_areas(floor: &Floor) -> Vec<f64> {
    let w = floor.params.width.max(1.0);
    floor
        .nodes
        .iter()
        .map(|n| match n {
            Node::Room { room } => floor.rooms[*room].area.max(1.0),
            Node::Junction { .. } => w * w,
        })
        .collect()
}

fn adjacency(n: usize, floor: &Floor) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for e in &floor.edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    adj
}

/// Where each node's water goes next: its lowest strictly lower neighbour (the lowest index on a tie), or itself at a sink.
fn drain(h: &[f64], adj: &[Vec<usize>]) -> Vec<usize> {
    (0..h.len())
        .map(|i| {
            adj[i]
                .iter()
                .copied()
                .filter(|&v| h[v] < h[i])
                .min_by(|&a, &b| h[a].total_cmp(&h[b]).then(a.cmp(&b)))
                .unwrap_or(i)
        })
        .collect()
}

fn sink_of(mut i: usize, next: &[usize]) -> usize {
    while next[i] != i {
        i = next[i];
    }
    i
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The volume a set of nodes (sorted by height) holds when the water stands at `level`.
fn capacity(sorted: &[usize], h: &[f64], area: &[f64], level: f64) -> f64 {
    sorted
        .iter()
        .take_while(|&&i| h[i] < level)
        .map(|&i| area[i] * (level - h[i]))
        .sum()
}

/// The level at which a set of nodes (sorted by height) holds `volume`.
fn level_for(sorted: &[usize], h: &[f64], area: &[f64], volume: f64) -> f64 {
    let (mut a, mut b) = (0.0, 0.0);
    for (k, &i) in sorted.iter().enumerate() {
        a += area[i];
        b += area[i] * h[i];
        let level = (volume + b) / a;
        if sorted.get(k + 1).is_none_or(|&next| level <= h[next]) {
            return level;
        }
    }
    h[sorted[0]]
}

/// The water poured by `sources` into the floor with heights `h`: the level at every node.
fn fill(
    h: &[f64],
    area: &[f64],
    adj: &[Vec<usize>],
    floor: &Floor,
    poured: &[(usize, f64)],
) -> Vec<f64> {
    let n = h.len();
    let next = drain(h, adj);
    let mut parent: Vec<usize> = (0..n).map(|i| sink_of(i, &next)).collect();
    // Representatives are sinks; `parent` of a sink is itself, of any other node its sink.
    for i in 0..n {
        let s = parent[i];
        parent[s] = s;
    }
    let mut volume = vec![0.0; n];
    for &(node, v) in poured {
        let r = find(&mut parent, node);
        volume[r] += v;
    }
    let mut full: Vec<Option<f64>> = vec![None; n];
    let eps = |x: f64| 1e-9 * (1.0 + x.abs());
    'again: for _ in 0..(n * n + 16) {
        let reps: Vec<usize> = (0..n).filter(|&i| find(&mut parent, i) == i).collect();
        for r in reps {
            let mut members: Vec<usize> = (0..n).filter(|&i| find(&mut parent, i) == r).collect();
            members.sort_by(|&a, &b| h[a].total_cmp(&h[b]).then(a.cmp(&b)));
            // The lowest saddle to a neighbouring set, ties to the lowest corridor.
            let mut saddle: Option<(f64, usize, usize)> = None;
            for (k, e) in floor.edges.iter().enumerate() {
                let (ru, rv) = (find(&mut parent, e.u), find(&mut parent, e.v));
                let (inside, outside) = match (ru == r, rv == r) {
                    (true, false) => (e.u, e.v),
                    (false, true) => (e.v, e.u),
                    _ => continue,
                };
                let s = h[inside].max(h[outside]);
                if saddle.is_none_or(|(best, _, _)| s < best) {
                    let rest = find(&mut parent, outside);
                    saddle = Some((s, k, rest));
                }
            }
            let Some((s, _, c)) = saddle else {
                continue;
            };
            let held = capacity(&members, h, area, s);
            if volume[r] > held + eps(held) {
                let excess = volume[r] - held;
                match full[c] {
                    // The neighbour already stands as high as this saddle: one pool.
                    Some(level) if level >= s - eps(s) => {
                        let (keep, drop) = (r.min(c), r.max(c));
                        parent[drop] = keep;
                        volume[keep] = volume[r] + volume[c];
                        if keep != r {
                            volume[r] = 0.0;
                        }
                        full[keep] = None;
                        full[drop] = None;
                    }
                    _ => {
                        volume[r] = held;
                        volume[c] += excess;
                        full[r] = Some(s);
                    }
                }
                continue 'again;
            } else if volume[r] < held - eps(held) {
                full[r] = None;
            }
        }
        break;
    }
    let mut level: Vec<f64> = h.to_vec();
    let reps: Vec<usize> = (0..n).filter(|&i| find(&mut parent, i) == i).collect();
    for r in reps {
        let mut members: Vec<usize> = (0..n).filter(|&i| find(&mut parent, i) == r).collect();
        members.sort_by(|&a, &b| h[a].total_cmp(&h[b]).then(a.cmp(&b)));
        if volume[r] > 0.0 {
            let l = level_for(&members, h, area, volume[r]);
            for &i in &members {
                level[i] = level[i].max(l);
            }
        }
    }
    level
}

/// The layer: it runs after the layout, and after the height layer it reads.
pub struct WaterLayer(pub WaterSpec);

impl WaterLayer {
    fn field(&self, floor: &Floor, heights: &[f64], rng: &mut Mulberry) -> WaterField {
        let spec = self.0;
        let n = floor.nodes.len();
        let area = node_areas(floor);
        let adj = adjacency(n, floor);
        let next = drain(heights, &adj);
        // The sources: rooms drawn without repeats, the higher the likelier.
        let mut rooms: Vec<usize> = (0..n)
            .filter(|&i| matches!(floor.nodes[i], Node::Room { room } if floor.rooms[room].alive))
            .collect();
        let lowest = heights.iter().copied().fold(f64::INFINITY, f64::min);
        let mean_room_area =
            rooms.iter().map(|&i| area[i]).sum::<f64>() / rooms.len().max(1) as f64;
        let mut sources = Vec::new();
        while sources.len() < spec.sources && !rooms.is_empty() {
            let weights: Vec<f64> = rooms.iter().map(|&i| heights[i] - lowest + 1.0).collect();
            let total: f64 = weights.iter().sum();
            let mut x = rng.next() * total;
            let mut pick = rooms.len() - 1;
            for (k, w) in weights.iter().enumerate() {
                if x < *w {
                    pick = k;
                    break;
                }
                x -= w;
            }
            sources.push(rooms.remove(pick));
        }
        let each = spec.flow * mean_room_area;
        let poured: Vec<(usize, f64)> =
            sources.iter().map(|&s| (sink_of(s, &next), each)).collect();
        let routes: Vec<Vec<usize>> = sources
            .iter()
            .map(|&s| {
                let mut path = vec![s];
                while next[*path.last().unwrap()] != *path.last().unwrap() {
                    path.push(next[*path.last().unwrap()]);
                }
                path
            })
            .collect();
        let level = fill(heights, &area, &adj, floor, &poured);
        let wet: Vec<bool> = level
            .iter()
            .zip(heights)
            .map(|(l, h)| l - h > 1e-9 * (1.0 + h.abs()))
            .collect();
        WaterField {
            humidity: super::fade_from(floor, &wet, spec.reach),
            level,
            routes,
            volume: each * sources.len() as f64,
            sources,
        }
    }
}

impl Layer for WaterLayer {
    fn name(&self) -> &'static str {
        "water"
    }
    fn tag(&self) -> u32 {
        // "WATR"
        0x5741_5452
    }
    fn phase(&self) -> LayerPhase {
        LayerPhase::After
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        // The water needs the ground: without the height layer there is nothing to run down (`Params::intent_error` says so).
        let Some(height) = out.height.as_ref() else {
            return;
        };
        let heights = height.nodes.clone();
        out.water = Some(self.field(floor, &heights, rng));
    }
}

/// The validator's check of the water (E2): the surface is never below the floor, the water held is the water poured, and a wet
/// node's surface is never above a neighbour's.
pub fn check(d: &Floor, field: &WaterField, heights: &[f64]) -> Vec<String> {
    let mut out = Vec::new();
    let n = d.nodes.len();
    if field.level.len() != n || field.humidity.len() != n || heights.len() != n {
        out.push("E2 water: there is not one level for each node".to_string());
        return out;
    }
    let eps = |x: f64| 1e-9 * (1.0 + x.abs());
    for (i, (level, h)) in field.level.iter().zip(heights).enumerate() {
        if *level < h - eps(*h) {
            out.push(format!("E2 water: node {i} has water below its floor"));
        }
    }
    let area = node_areas(d);
    let held: f64 = (0..n)
        .map(|i| area[i] * (field.level[i] - heights[i]).max(0.0))
        .sum();
    if (held - field.volume).abs() > 1e-6 * (1.0 + field.volume) {
        out.push(format!(
            "E2 water: the floor holds {held:.1} of the {:.1} poured",
            field.volume
        ));
    }
    for e in &d.edges {
        for (a, b) in [(e.u, e.v), (e.v, e.u)] {
            let wet = field.level[a] - heights[a] > eps(heights[a]);
            if wet && field.level[a] > field.level[b] + 1e-6 * (1.0 + field.level[a].abs()) {
                out.push(format!(
                    "E2 water: the surface at node {a} stands above its neighbour {b}"
                ));
            }
        }
    }
    out
}
