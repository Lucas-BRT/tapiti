//! Stacked levels: several floors in the same world, each generated on its own with the whole pipeline, joined by stairs.
//!
//! Level 0 uses the seed and level $k$ an integer mix of it, so nothing leaks from one level to the next. Rooms of different levels may
//! overlap in plan (that is the point), so no level is moved: a stair stands where a room of level $k$ and a room of level $k+1$
//! overlap most. A stair is an edge of the graph with no doors and no geometry; every geometric rule of the validator is checked on
//! each level alone ([`level_floor`]), and the stairs have rules of their own. The environment layers run once, on the whole.

use super::*;
use crate::{cancel::Cancel, environment, topology::shortest_path};

/// The seed of level `k`: the seed itself for level 0, else an integer mix of both.
pub fn level_seed(seed: u32, k: usize) -> u32 {
    if k == 0 {
        return seed;
    }
    let mut z = seed ^ (k as u32).wrapping_mul(0x9e37_79b9);
    z = (z ^ (z >> 16)).wrapping_mul(0x85eb_ca6b);
    z = (z ^ (z >> 13)).wrapping_mul(0xc2b2_ae35);
    z ^ (z >> 16)
}

/// Λ of one level: the same, with one level and no environment layers (those run on the whole).
fn level_params(p: &Params) -> Params {
    let mut q = p.clone();
    q.levels = LevelsSpec::default();
    q.environment = Default::default();
    q
}

pub(super) fn generate_stack(
    seed: u32,
    p: &Params,
    mut progress: Option<&mut dyn FnMut(GenerationEvent)>,
    cancel: Option<Cancel>,
) -> Option<Floor> {
    let q = level_params(p);
    let mut levels = Vec::new();
    for k in 0..p.levels.count {
        // Only level 0 is drawn while it is built: the edge ids of the others would overwrite its own.
        let reporter: Option<&mut dyn FnMut(GenerationEvent)> = if k == 0 {
            progress
                .as_mut()
                .map(|f| &mut **f as &mut dyn FnMut(GenerationEvent))
        } else {
            None
        };
        levels.push(pipeline::generate_level(
            level_seed(seed, k),
            &q,
            reporter,
            cancel.clone(),
            false,
        )?);
    }
    Some(assemble(seed, p, levels))
}

/// How much two boxes overlap in plan, as `(width, height)`, 0 when they do not.
fn overlap(a: &Room, b: &Room) -> (f64, f64) {
    let w = a.bounds.hi[0].min(b.bounds.hi[0]) - a.bounds.lo[0].max(b.bounds.lo[0]);
    let h = a.bounds.hi[1].min(b.bounds.hi[1]) - a.bounds.lo[1].max(b.bounds.lo[1]);
    (w.max(0.0), h.max(0.0))
}

/// The rooms a stair may end in: alive, not a leaf chamber, not the entrance or the exit of their level.
fn stair_room(r: &Room) -> bool {
    r.alive && !r.leaf && r.role == Role::Plain
}

/// The stairs between level `k` and `k + 1`: up to `count`, each where two rooms overlap most (both ways at least `2 width`), the
/// first by index on a tie, no room twice. A pair with no overlap that big gets none.
fn stairs_between(rooms: &[Room], k: usize, count: usize, width: f64) -> Vec<(usize, usize, V)> {
    let mut used: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for _ in 0..count {
        let mut best: Option<(f64, usize, usize, V)> = None;
        for a in rooms
            .iter()
            .filter(|r| r.level == k && stair_room(r) && !used.contains(&r.id))
        {
            for b in rooms
                .iter()
                .filter(|r| r.level == k + 1 && stair_room(r) && !used.contains(&r.id))
            {
                let (w, h) = overlap(a, b);
                if w >= 2.0 * width
                    && h >= 2.0 * width
                    && best.is_none_or(|(area, ..)| w * h > area)
                {
                    let x = (a.bounds.lo[0].max(b.bounds.lo[0])
                        + a.bounds.hi[0].min(b.bounds.hi[0]))
                        / 2.0;
                    let y = (a.bounds.lo[1].max(b.bounds.lo[1])
                        + a.bounds.hi[1].min(b.bounds.hi[1]))
                        / 2.0;
                    best = Some((w * h, a.id, b.id, [x, y]));
                }
            }
        }
        let Some((_, a, b, at)) = best else {
            break;
        };
        used.extend([a, b]);
        out.push((a, b, at));
    }
    out
}

fn assemble(seed: u32, p: &Params, floors: Vec<Floor>) -> Floor {
    let started = crate::clock::now_ms();
    let mut rooms: Vec<Room> = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut infos: Vec<LevelInfo> = Vec::new();
    let mut log: Vec<String> = Vec::new();
    let shift = |q: &mut Port, by: usize| q.room += by;
    for (k, f) in floors.iter().enumerate() {
        let (ro, no, eo) = (rooms.len(), nodes.len(), edges.len());
        for r in &f.rooms {
            let mut r = r.clone();
            r.id += ro;
            r.level = k;
            r.ports
                .iter_mut()
                .chain(r.used.iter_mut())
                .for_each(|q| shift(q, ro));
            rooms.push(r);
        }
        for n in &f.nodes {
            nodes.push(match n {
                Node::Room { room } => Node::Room { room: room + ro },
                other => other.clone(),
            });
        }
        for e in &f.edges {
            let mut e = e.clone();
            e.id += eo;
            e.u += no;
            e.v += no;
            e.ends.iter_mut().flatten().for_each(|q| shift(q, ro));
            edges.push(e);
        }
        infos.push(LevelInfo {
            room_offset: ro,
            rooms: f.rooms.len(),
            node_offset: no,
            nodes: f.nodes.len(),
            edge_offset: eo,
            edges: f.edges.len(),
            entrance: f.entrance.map(|r| r + ro),
            exit: f.exit.map(|r| r + ro),
            main_path: f.main_path.iter().map(|n| n + no).collect(),
        });
        log.extend(f.log.iter().map(|l| format!("level {}: {l}", k + 1)));
    }
    let node_of: HashMap<usize, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n {
            Node::Room { room } => Some((*room, i)),
            _ => None,
        })
        .collect();
    for k in 0..floors.len().saturating_sub(1) {
        let found = stairs_between(&rooms, k, p.levels.stairs, p.width);
        if found.is_empty() {
            log.push(format!(
                "no room of level {} overlaps one of level {} enough for a stair",
                k + 1,
                k + 2
            ));
        }
        for (a, b, at) in found {
            let id = edges.len();
            edges.push(Edge {
                id,
                u: node_of[&a],
                v: node_of[&b],
                pts: vec![at, at],
                ends: [None, None],
                stair: true,
                region: Vec::new(),
            });
        }
    }
    let entrance = infos.first().and_then(|i| i.entrance);
    let exit = infos.last().and_then(|i| i.exit);
    let main_path = entrance
        .zip(exit)
        .and_then(|(a, b)| shortest_path(nodes.len(), &edges, node_of[&a], node_of[&b]))
        .map(|(path, _)| path)
        .unwrap_or_default();
    let first = &floors[0];
    let mut phases = first.phases.clone();
    phases.push((
        "other levels".to_string(),
        floors[1..]
            .iter()
            .flat_map(|f| f.phases.iter())
            .map(|(_, t)| t)
            .sum::<f64>(),
    ));
    let mut floor = Floor {
        params: p.clone(),
        seed,
        rooms,
        nodes,
        edges,
        entrance,
        exit,
        main_path,
        grid_x: first.grid_x.clone(),
        grid_y: first.grid_y.clone(),
        beta_max: first.beta_max,
        beta_target: first.beta_target,
        candidates: Vec::new(),
        pairs: first.pairs.clone(),
        dropped: first.dropped.clone(),
        log,
        environment: None,
        stack: Some(StackInfo { levels: infos }),
        phases,
    };
    floor.candidates = first.candidates.clone();
    phases_push(&mut floor, started);
    let layers = environment::layers_of(p);
    floor.environment = environment::run_after(&layers, seed, &floor, None);
    floor
}

fn phases_push(floor: &mut Floor, started: f64) {
    floor
        .phases
        .push(("stairs".to_string(), crate::clock::now_ms() - started));
}

/// Level `k` of a stacked floor as a floor of its own, with its ids from 0: what `generate` draws for the seed of that level.
pub fn level_floor(d: &Floor, k: usize) -> Floor {
    let info = &d.stack.as_ref().expect("a stacked floor").levels[k];
    let (ro, no, eo) = (info.room_offset, info.node_offset, info.edge_offset);
    let shift = |q: &mut Port| q.room -= ro;
    let rooms = d.rooms[ro..ro + info.rooms]
        .iter()
        .map(|r| {
            let mut r = r.clone();
            r.id -= ro;
            r.level = 0;
            r.ports.iter_mut().chain(r.used.iter_mut()).for_each(shift);
            r
        })
        .collect();
    let nodes = d.nodes[no..no + info.nodes]
        .iter()
        .map(|n| match n {
            Node::Room { room } => Node::Room { room: room - ro },
            other => other.clone(),
        })
        .collect();
    let edges = d.edges[eo..eo + info.edges]
        .iter()
        .map(|e| {
            let mut e = e.clone();
            e.id -= eo;
            e.u -= no;
            e.v -= no;
            e.ends.iter_mut().flatten().for_each(shift);
            e
        })
        .collect();
    Floor {
        params: level_params(&d.params),
        seed: level_seed(d.seed, k),
        rooms,
        nodes,
        edges,
        entrance: info.entrance.map(|r| r - ro),
        exit: info.exit.map(|r| r - ro),
        main_path: info.main_path.iter().map(|n| n - no).collect(),
        grid_x: d.grid_x.clone(),
        grid_y: d.grid_y.clone(),
        beta_max: d.beta_max,
        beta_target: d.beta_target,
        candidates: Vec::new(),
        pairs: Vec::new(),
        dropped: Vec::new(),
        log: Vec::new(),
        environment: None,
        stack: None,
        phases: Vec::new(),
    }
}

/// The validator of a stacked floor: every level alone against the whole of the rules, then the stairs (V7) and the walk from the
/// entrance to every room over all of them (V1).
pub(crate) fn validate_stack(d: &Floor) -> Vec<String> {
    let stack = d.stack.as_ref().expect("a stacked floor");
    let mut out: Vec<String> = Vec::new();
    if let Some(error) = d.params.intent_error() {
        out.push(format!("T6 {error}"));
    }
    for k in 0..stack.levels.len() {
        out.extend(
            crate::validate::validate(&level_floor(d, k))
                .into_iter()
                .map(|v| format!("level {}: {v}", k + 1)),
        );
    }
    let width = d.params.width;
    let mut per_pair = vec![0usize; stack.levels.len().saturating_sub(1)];
    for e in d.edges.iter().filter(|e| e.stair) {
        let rooms: Vec<&Room> = [e.u, e.v]
            .iter()
            .filter_map(|n| match d.nodes[*n] {
                Node::Room { room } => Some(&d.rooms[room]),
                _ => None,
            })
            .collect();
        let [a, b] = rooms[..] else {
            out.push(format!("V7 stair {} does not join two rooms", e.id));
            continue;
        };
        let (lo, hi) = (a.level.min(b.level), a.level.max(b.level));
        if hi != lo + 1 {
            out.push(format!(
                "V7 stair {} joins levels {} and {}, which are not consecutive",
                e.id,
                lo + 1,
                hi + 1
            ));
            continue;
        }
        per_pair[lo] += 1;
        let (w, h) = overlap(a, b);
        if w < 2.0 * width || h < 2.0 * width {
            out.push(format!(
                "V7 stair {}: rooms {} and {} do not overlap enough",
                e.id, a.id, b.id
            ));
        }
        if !(stair_room(a) && stair_room(b)) {
            out.push(format!(
                "V7 stair {} ends in an entrance, an exit or a leaf chamber",
                e.id
            ));
        }
    }
    for (k, n) in per_pair.iter().enumerate() {
        if *n == 0 {
            out.push(format!(
                "V7 levels {} and {} are not joined by a stair",
                k + 1,
                k + 2
            ));
        }
        if *n > d.params.levels.stairs {
            out.push(format!(
                "V7 levels {} and {} have more stairs than asked",
                k + 1,
                k + 2
            ));
        }
    }
    // V1 over the whole: every room is reached from the entrance of the first level.
    let node_of: HashMap<usize, usize> = d
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n {
            Node::Room { room } => Some((*room, i)),
            _ => None,
        })
        .collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); d.nodes.len()];
    for e in &d.edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    if let Some(&start) = d.entrance.and_then(|r| node_of.get(&r)) {
        let mut seen = vec![false; d.nodes.len()];
        seen[start] = true;
        let mut todo = vec![start];
        while let Some(u) = todo.pop() {
            for &v in &adj[u] {
                if !seen[v] {
                    seen[v] = true;
                    todo.push(v);
                }
            }
        }
        for r in d.rooms.iter().filter(|r| r.alive) {
            if !seen[node_of[&r.id]] {
                out.push(format!(
                    "V1 room {} (level {}) not reached from the entrance",
                    r.id,
                    r.level + 1
                ));
            }
        }
    }
    out.extend(crate::validate::environment_violations(d));
    out
}
