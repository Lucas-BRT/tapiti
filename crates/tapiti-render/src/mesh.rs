//! The floor as triangles: floors, walls with door openings, corridors that climb between the heights of their ends, stairs, and the
//! water over the wet rooms. A renderer (the desktop's 3D view) uploads it as it is: positions are `[x, up, plan y]`, with the height
//! of the floor as the `up` axis. The same floor and options give the same mesh.

use crate::view::{self, Channel};
use std::collections::HashMap;
use tapiti_core::generate::{Floor, Node, Room};
use tapiti_core::geom::{dist, Shape, V};

/// Triangles with a normal and a colour at each vertex.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[u8; 3]>,
    pub indices: Vec<u32>,
    /// The room each triangle belongs to (`NO_ROOM` for a corridor or a stair), one for every three indices: for picking.
    pub owner: Vec<u32>,
}

/// The owner of the triangles that belong to no room.
pub const NO_ROOM: u32 = u32::MAX;

impl Mesh {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    fn quad(&mut self, p: [[f32; 3]; 4], color: [u8; 3], owner: u32) {
        let n = normalize3(cross3(sub3(p[1], p[0]), sub3(p[3], p[0])));
        let base = self.positions.len() as u32;
        for v in p {
            self.positions.push(v);
            self.normals.push(n);
            self.colors.push(color);
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        self.owner.extend([owner, owner]);
    }

    /// A flat polygon at height `y` facing up (`up` true) or down, triangulated by ear clipping.
    fn flat(&mut self, pts: &[V], y: f32, color: [u8; 3], up: bool, owner: u32) {
        let n = if up {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, -1.0, 0.0]
        };
        let base = self.positions.len() as u32;
        for p in pts {
            self.positions.push([p[0] as f32, y, p[1] as f32]);
            self.normals.push(n);
            self.colors.push(color);
        }
        for t in triangulate(pts) {
            self.indices
                .extend([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
            self.owner.push(owner);
        }
    }
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// Ear clipping, for concave rooms (L, star): the triangles of a simple polygon as indices into its points.
pub fn triangulate(pts: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sign
    };
    let mut ring: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    'clip: while ring.len() > 3 {
        let m = ring.len();
        for i in 0..m {
            let (p, c, q) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            if cross(pts[p], pts[c], pts[q]) <= 1e-9 {
                continue;
            }
            let blocked = ring.iter().any(|&o| {
                if [p, c, q].iter().any(|&v| pts[v] == pts[o]) {
                    return false;
                }
                cross(pts[p], pts[c], pts[o]) >= 0.0
                    && cross(pts[c], pts[q], pts[o]) >= 0.0
                    && cross(pts[q], pts[p], pts[o]) >= 0.0
            });
            if !blocked {
                out.push([p, c, q]);
                ring.remove(i);
                continue 'clip;
            }
        }
        break; // degenerate ring: fan the rest below
    }
    for i in 1..ring.len().saturating_sub(1) {
        out.push([ring[0], ring[i], ring[i + 1]]);
    }
    out
}

/// The polygon of a room's outline (a disc as 32 points).
pub fn outline(shape: &Shape) -> Vec<V> {
    match shape {
        Shape::Disc { c, r } => (0..32)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 32.0;
                [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
            })
            .collect(),
        Shape::Poly { pts } => pts.clone(),
    }
}

/// The height of every node of the floor: its own when there is a height layer, else one storey per level.
pub fn node_heights(floor: &Floor) -> Vec<f64> {
    match floor.environment.as_ref().and_then(|e| e.height.as_ref()) {
        Some(h) => h.nodes.clone(),
        None => {
            let storey = floor.params.levels.storey;
            tapiti_core::environment::height::node_levels(floor)
                .iter()
                .map(|l| *l as f64 * storey)
                .collect()
        }
    }
}

/// What the mesh is built for.
#[derive(Clone, Copy, Debug)]
pub struct MeshOptions {
    /// The height of the walls.
    pub wall_height: f64,
    /// How much taller than life the heights are drawn (the walls are not scaled).
    pub exaggeration: f64,
    /// Extra distance between levels, as a fraction of a storey.
    pub explode: f64,
    /// The highest level built, counted from 0; `None` builds them all.
    pub up_to: Option<usize>,
    /// Roofs over the rooms and corridors, for a view from inside.
    pub ceilings: bool,
    /// Tint the floors of the rooms by a channel.
    pub tint: Option<Channel>,
    /// Darken the rooms the light does not reach.
    pub light: bool,
    /// Build the water surfaces over the wet rooms.
    pub water: bool,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            wall_height: 60.0,
            exaggeration: 1.0,
            explode: 0.5,
            up_to: None,
            ceilings: false,
            tint: None,
            light: false,
            water: true,
        }
    }
}

/// The meshes of a floor: the solid parts and the water (drawn after, blended), with the box they fill.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub solid: Mesh,
    pub water: Mesh,
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// The colour of a class of room.
pub fn class_color(class: &str) -> [u8; 3] {
    match class {
        "rect" => [0x71, 0x9a, 0xb0],
        "disc" => [0x94, 0xb4, 0x9d],
        "oct" => [0xcd, 0xbb, 0xd2],
        "L" => [0xbb, 0x8d, 0x58],
        "star" => [0xe8, 0xd0, 0x7d],
        _ => [0x59, 0x67, 0x7a],
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    [0, 1, 2].map(|i| (f64::from(a[i]) + (f64::from(b[i]) - f64::from(a[i])) * t).round() as u8)
}

fn shade(c: [u8; 3], k: f64) -> [u8; 3] {
    [0, 1, 2].map(|i| (f64::from(c[i]) * k).clamp(0.0, 255.0) as u8)
}

/// Where along the edge `a`-`b` (0 to 1) a door at `p` stands, when it stands on the edge (within a hair of it).
fn door_t(a: V, b: V, p: V) -> Option<f64> {
    let len = dist(a, b);
    if len <= 1e-9 {
        return None;
    }
    let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len;
    let off = ((p[0] - a[0]) * d[1] - (p[1] - a[1]) * d[0]).abs();
    (off <= 1.5 && (-0.01..=1.01).contains(&t)).then_some(t)
}

/// The pieces of the edge `a`-`b` of a wall left once the doors on it are cut out, as `(from, to)` along the edge in 0 to 1. A door is a
/// used port within a hair of the edge, `width` wide.
pub fn wall_pieces(a: V, b: V, doors: &[V], width: f64) -> Vec<(f64, f64)> {
    let len = dist(a, b);
    if len <= 1e-9 {
        return Vec::new();
    }
    let mut gaps: Vec<(f64, f64)> = doors
        .iter()
        .filter_map(|p| door_t(a, b, *p))
        .map(|t| {
            (
                (t - width / 2.0 / len).max(0.0),
                (t + width / 2.0 / len).min(1.0),
            )
        })
        .collect();
    gaps.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for (g0, g1) in gaps {
        if g0 > at {
            out.push((at, g0));
        }
        at = at.max(g1);
    }
    if at < 1.0 {
        out.push((at, 1.0));
    }
    out
}

/// Builds the mesh of a floor.
pub fn build(floor: &Floor, o: &MeshOptions) -> Scene {
    let node_level = tapiti_core::environment::height::node_levels(floor);
    let heights = node_heights(floor);
    let storey = floor.params.levels.storey;
    let shown = |level: usize| {
        o.up_to
            .filter(|_| floor.stack.is_some())
            .is_none_or(|u| level <= u)
    };
    let y_of = |node: usize| {
        ((heights[node] + node_level[node] as f64 * o.explode * storey) * o.exaggeration) as f32
    };
    let wall = o.wall_height as f32;
    let door = (o.wall_height * 0.7) as f32;
    let width = floor.params.width.max(2.0);
    let half = width / 2.0;
    let room_node: HashMap<usize, usize> = floor
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n {
            Node::Room { room } => Some((*room, i)),
            _ => None,
        })
        .collect();
    let tint = o
        .tint
        .and_then(|c| view::scale(floor, c).map(|s| (c, s, view::room_values(floor, c))));
    let lights = o.light.then(|| view::room_values(floor, Channel::Light));
    let depth = o.water.then(|| view::room_values(floor, Channel::Water));
    let mut s = Scene::default();
    let corridor = [0x85, 0x7d, 0x70];

    for r in floor.rooms.iter().filter(|r| r.alive && shown(r.level)) {
        let node = room_node[&r.id];
        let y = y_of(node);
        let mut base = class_color(r.cls.name());
        if let Some((c, scale, values)) = &tint {
            if let Some(v) = values[r.id] {
                base = mix(base, view::color(*c, view::position(*scale, v)), 0.55);
            }
        }
        if let Some(Some(l)) = lights.as_ref().map(|v| v[r.id]) {
            base = shade(base, 0.25 + 0.75 * l);
        }
        let pts = outline(&r.shape);
        if pts.len() < 3 {
            continue;
        }
        s.solid.flat(&pts, y, base, true, r.id as u32);
        if o.ceilings {
            s.solid
                .flat(&pts, y + wall, shade(base, 0.8), false, r.id as u32);
        }
        let doors: Vec<V> = r.used.iter().map(|q| q.p).collect();
        let wall_color = shade(base, 0.85);
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            for (t0, t1) in wall_pieces(a, b, &doors, width) {
                let (p, q) = (at(t0), at(t1));
                s.solid.quad(
                    [
                        [p[0] as f32, y, p[1] as f32],
                        [q[0] as f32, y, q[1] as f32],
                        [q[0] as f32, y + wall, q[1] as f32],
                        [p[0] as f32, y + wall, p[1] as f32],
                    ],
                    wall_color,
                    r.id as u32,
                );
            }
            // The lintel over each door, so the opening has a top.
            for t in doors.iter().filter_map(|d| door_t(a, b, *d)) {
                let len = dist(a, b);
                let (p, q) = (at((t - half / len).max(0.0)), at((t + half / len).min(1.0)));
                if wall > door {
                    s.solid.quad(
                        [
                            [p[0] as f32, y + door, p[1] as f32],
                            [q[0] as f32, y + door, q[1] as f32],
                            [q[0] as f32, y + wall, q[1] as f32],
                            [p[0] as f32, y + wall, p[1] as f32],
                        ],
                        wall_color,
                        r.id as u32,
                    );
                }
            }
        }
        if let Some(Some(w)) = depth.as_ref().map(|v| v[r.id]) {
            if w > 1e-9 {
                let level = floor
                    .environment
                    .as_ref()
                    .and_then(|e| e.water.as_ref())
                    .map_or(0.0, |wt| wt.level[node]);
                let wy = ((level + node_level[node] as f64 * o.explode * storey) * o.exaggeration)
                    as f32;
                s.water
                    .flat(&pts, wy, [0x1f, 0x7f, 0xc9], true, r.id as u32);
            }
        }
    }

    // Corridors: the floor of each segment climbs from the height of one end to the other's, with a wall on each side.
    for e in floor
        .edges
        .iter()
        .filter(|e| !e.stair && shown(node_level[e.u]))
    {
        let total: f64 = e
            .pts
            .windows(2)
            .map(|p| dist(p[0], p[1]))
            .sum::<f64>()
            .max(1e-9);
        let (y0, y1) = (y_of(e.u), y_of(e.v));
        let mut run = 0.0;
        for p in e.pts.windows(2) {
            let len = dist(p[0], p[1]);
            if len <= 0.0 {
                continue;
            }
            let (ya, yb) = (
                y0 + (y1 - y0) * (run / total) as f32,
                y0 + (y1 - y0) * ((run + len) / total) as f32,
            );
            run += len;
            let n = [
                -(p[1][1] - p[0][1]) / len * half,
                (p[1][0] - p[0][0]) / len * half,
            ];
            let corner = |pt: V, sgn: f64, y: f32| {
                [(pt[0] + sgn * n[0]) as f32, y, (pt[1] + sgn * n[1]) as f32]
            };
            s.solid.quad(
                [
                    corner(p[0], 1.0, ya),
                    corner(p[1], 1.0, yb),
                    corner(p[1], -1.0, yb),
                    corner(p[0], -1.0, ya),
                ],
                corridor,
                NO_ROOM,
            );
            for sgn in [1.0, -1.0] {
                let (l0, l1) = (corner(p[0], sgn, ya), corner(p[1], sgn, yb));
                s.solid.quad(
                    [
                        l0,
                        l1,
                        [l1[0], l1[1] + wall, l1[2]],
                        [l0[0], l0[1] + wall, l0[2]],
                    ],
                    shade(corridor, 0.8),
                    NO_ROOM,
                );
            }
            if o.ceilings {
                s.solid.quad(
                    [
                        [
                            corner(p[0], 1.0, ya)[0],
                            ya + wall,
                            corner(p[0], 1.0, ya)[2],
                        ],
                        [
                            corner(p[1], 1.0, yb)[0],
                            yb + wall,
                            corner(p[1], 1.0, yb)[2],
                        ],
                        [
                            corner(p[1], -1.0, yb)[0],
                            yb + wall,
                            corner(p[1], -1.0, yb)[2],
                        ],
                        [
                            corner(p[0], -1.0, ya)[0],
                            ya + wall,
                            corner(p[0], -1.0, ya)[2],
                        ],
                    ],
                    shade(corridor, 0.7),
                    NO_ROOM,
                );
            }
        }
    }

    // Stairs: a flight of steps from the lower floor to the upper one, at the point of the stair.
    for e in floor.edges.iter().filter(|e| e.stair) {
        let (lu, lv) = (node_level[e.u], node_level[e.v]);
        if !(shown(lu) && shown(lv)) {
            continue;
        }
        let at = e.pts[0];
        let (yu, yv) = (y_of(e.u), y_of(e.v));
        let (lo, hi) = (yu.min(yv), yu.max(yv));
        let steps = 8;
        for i in 0..steps {
            let (ya, yb) = (
                lo + (hi - lo) * i as f32 / steps as f32,
                lo + (hi - lo) * (i + 1) as f32 / steps as f32,
            );
            let x0 = (at[0] - 24.0 + 6.0 * f64::from(i)) as f32;
            let (z0, z1) = ((at[1] - 12.0) as f32, (at[1] + 12.0) as f32);
            s.solid.quad(
                [
                    [x0, yb, z0],
                    [x0 + 6.0, yb, z0],
                    [x0 + 6.0, yb, z1],
                    [x0, yb, z1],
                ],
                [0xb4, 0x9a, 0x7a],
                NO_ROOM,
            );
            s.solid.quad(
                [[x0, ya, z0], [x0, yb, z0], [x0, yb, z1], [x0, ya, z1]],
                [0x8f, 0x78, 0x5c],
                NO_ROOM,
            );
        }
    }

    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in s.solid.positions.iter().chain(&s.water.positions) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0].is_finite() {
        s.lo = lo;
        s.hi = hi;
    }
    s
}

/// The room a point of the plan is in, if any: for a view that walks the floor.
pub fn room_at(floor: &Floor, p: V, level: Option<usize>) -> Option<&Room> {
    floor.rooms.iter().rev().find(|r| {
        r.alive && level.is_none_or(|l| r.level == l) && {
            let poly = outline(&r.shape);
            let mut inside = false;
            let mut j = poly.len().saturating_sub(1);
            for i in 0..poly.len() {
                let ([xi, yi], [xj, yj]) = (poly[i], poly[j]);
                if (yi > p[1]) != (yj > p[1]) && p[0] < (xj - xi) * (p[1] - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{
        generate::generate,
        params::{rich_of_size, Layout, Size},
    };

    fn full() -> Floor {
        let mut p = rich_of_size(Size::Medium);
        p.layout = Layout::Growth;
        p.levels.count = 2;
        p.levels.stairs = 2;
        p.environment.height.enabled = true;
        p.environment.water.enabled = true;
        p.environment.water.flow = 300.0;
        p.environment.light.enabled = true;
        generate(3, &p)
    }

    #[test]
    fn the_mesh_is_well_formed_and_repeats() {
        let f = full();
        let o = MeshOptions {
            tint: Some(Channel::Height),
            light: true,
            ceilings: true,
            ..MeshOptions::default()
        };
        let s = build(&f, &o);
        for m in [&s.solid, &s.water] {
            assert_eq!(m.positions.len(), m.normals.len());
            assert_eq!(m.positions.len(), m.colors.len());
            assert!(m.indices.iter().all(|i| (*i as usize) < m.positions.len()));
            assert!(m.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(m.normals.iter().flatten().all(|v| v.is_finite()));
        }
        assert!(s.solid.triangles() > f.rooms.len() * 4);
        assert!(s.water.triangles() > 0, "no water in a flooded floor");
        assert!((0..3).all(|k| s.lo[k] <= s.hi[k]));
        assert_eq!(s, build(&f, &o));
    }

    #[test]
    fn a_level_cut_builds_less_and_a_floor_without_water_has_none() {
        let f = full();
        let all = build(&f, &MeshOptions::default());
        let first = build(
            &f,
            &MeshOptions {
                up_to: Some(0),
                ..MeshOptions::default()
            },
        );
        assert!(first.solid.triangles() < all.solid.triangles());
        let dry = build(
            &f,
            &MeshOptions {
                water: false,
                ..MeshOptions::default()
            },
        );
        assert_eq!(dry.water.triangles(), 0);
    }

    #[test]
    fn doors_are_cut_out_of_the_walls() {
        let (a, b) = ([0.0, 0.0], [100.0, 0.0]);
        let whole = wall_pieces(a, b, &[], 20.0);
        assert_eq!(whole, vec![(0.0, 1.0)]);
        let cut = wall_pieces(a, b, &[[50.0, 0.0]], 20.0);
        let kept: f64 = cut.iter().map(|(x, y)| (y - x) * 100.0).sum();
        assert!((kept - 80.0).abs() < 1e-9, "{cut:?}");
        // A door on another wall is not cut out of this one, and two doors cut two gaps.
        assert_eq!(wall_pieces(a, b, &[[50.0, 40.0]], 20.0), vec![(0.0, 1.0)]);
        assert_eq!(
            wall_pieces(a, b, &[[20.0, 0.0], [80.0, 0.0]], 20.0).len(),
            3
        );
    }

    #[test]
    fn concave_polygons_are_covered_exactly() {
        let l = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 2.0],
            [2.0, 2.0],
            [2.0, 4.0],
            [0.0, 4.0],
        ];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), l.len() - 2);
        let area: f64 = tris
            .iter()
            .map(|t| {
                let (a, b, c) = (l[t[0]], l[t[1]], l[t[2]]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert!((area - 12.0).abs() < 1e-9);
    }
}
