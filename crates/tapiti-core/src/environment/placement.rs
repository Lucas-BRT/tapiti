//! Where things go on a floor: a distribution over the rooms that a layer draws its sources, events or openings from.
//!
//! Each placement gives every room a weight; `place` draws rooms without repeats, each in turn with a chance proportional to its weight,
//! from the stream of the layer. A placement that finds no room fit for it (a floor with no dead end, no main path, no height) falls
//! back to the uniform one, so asking for something always gives something. Weights use sums, products and quotients only.

use super::height::hops_from_entrance;
use crate::{
    generate::{Floor, Node},
    rng::{Mulberry, Rand},
};
use serde::{Deserialize, Serialize};

/// How rooms are weighted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Placement {
    /// Every room alike.
    #[default]
    Uniform,
    /// The nearer the entrance, in corridors, the likelier.
    NearEntrance,
    /// The farther from the entrance, the likelier.
    FarFromEntrance,
    /// Only rooms on the way from the entrance to the exit.
    MainPath,
    /// Only rooms with a single corridor.
    DeadEnds,
    /// The higher the floor, the likelier (needs the height layer).
    HighGround,
    /// The lower the floor, the likelier (needs the height layer).
    LowGround,
}

/// The weight of every node: 0 for a junction, a dead room, or a room the placement rules out.
pub fn weights(floor: &Floor, heights: Option<&[f64]>, placement: Placement) -> Vec<f64> {
    let n = floor.nodes.len();
    let live: Vec<bool> = floor
        .nodes
        .iter()
        .map(|node| matches!(node, Node::Room { room } if floor.rooms[*room].alive))
        .collect();
    let mut degree = vec![0usize; n];
    for e in &floor.edges {
        degree[e.u] += 1;
        degree[e.v] += 1;
    }
    let by = |f: &dyn Fn(usize) -> f64| -> Vec<f64> {
        (0..n).map(|i| if live[i] { f(i) } else { 0.0 }).collect()
    };
    let hops = hops_from_entrance(floor);
    let heights = heights.filter(|h| h.len() == n);
    let (lo, hi) = heights.map_or((0.0, 0.0), |h| {
        (
            h.iter().copied().fold(f64::INFINITY, f64::min),
            h.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    });
    let chosen = match placement {
        Placement::Uniform => by(&|_| 1.0),
        Placement::NearEntrance => by(&|i| 1.0 / (1.0 + hops[i] as f64)),
        Placement::FarFromEntrance => by(&|i| 1.0 + hops[i] as f64),
        Placement::MainPath => by(&|i| f64::from(floor.main_path.contains(&i))),
        Placement::DeadEnds => by(&|i| f64::from(degree[i] == 1)),
        Placement::HighGround => match heights {
            Some(h) => by(&|i| h[i] - lo + 1.0),
            None => by(&|_| 1.0),
        },
        Placement::LowGround => match heights {
            Some(h) => by(&|i| hi - h[i] + 1.0),
            None => by(&|_| 1.0),
        },
    };
    if chosen.iter().sum::<f64>() > 0.0 {
        chosen
    } else {
        by(&|_| 1.0)
    }
}

/// Draws up to `count` rooms without repeats, skipping the nodes in `exclude`, in the order drawn.
pub fn place(
    floor: &Floor,
    heights: Option<&[f64]>,
    placement: Placement,
    count: usize,
    exclude: &[usize],
    rng: &mut Mulberry,
) -> Vec<usize> {
    let mut w = weights(floor, heights, placement);
    for &i in exclude {
        w[i] = 0.0;
    }
    // A placement that leaves nothing once the excluded rooms are out falls back to the uniform one.
    if w.iter().sum::<f64>() <= 0.0 {
        w = weights(floor, heights, Placement::Uniform);
        for &i in exclude {
            w[i] = 0.0;
        }
    }
    let mut out = Vec::new();
    while out.len() < count {
        let total: f64 = w.iter().sum();
        if total <= 0.0 {
            break;
        }
        let mut x = rng.next() * total;
        let mut pick = w.iter().rposition(|v| *v > 0.0).unwrap_or(0);
        for (i, v) in w.iter().enumerate() {
            if *v > 0.0 && x < *v {
                pick = i;
                break;
            }
            x -= v;
        }
        out.push(pick);
        w[pick] = 0.0;
    }
    out
}
