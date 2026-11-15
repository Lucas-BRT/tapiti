//! The 2.5D view: the floor drawn obliquely with the painter, rooms as slabs at their height, corridors as ramps between them, levels
//! stacked, stairs as columns. No GPU: every shape is a polygon drawn from the far to the near, which is exact enough for a floor.

use crate::map::{event_color, Palette, Tints, Xf};
use crate::Overlays;
use eframe::egui::{Color32, Painter, Pos2, Shape as Draw, Stroke};
use tapiti_core::generate::{Floor, Node, Room};
use tapiti_core::geom::Shape;

/// Where the floor is looked at from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsoCamera {
    /// Turn about the vertical, in degrees.
    pub yaw: f64,
    /// Angle of the view above the plane, in degrees: 90 is straight down.
    pub tilt: f64,
    /// How much taller than life the heights are drawn.
    pub exaggeration: f64,
    /// Extra distance between levels, as a fraction of a storey.
    pub explode: f64,
    /// The highest level drawn, counted from 0, to look inside a stack; `None` draws them all.
    pub up_to: Option<usize>,
}

impl Default for IsoCamera {
    fn default() -> Self {
        Self {
            yaw: 35.0,
            tilt: 55.0,
            exaggeration: 2.0,
            explode: 0.5,
            up_to: None,
        }
    }
}

/// The thickness of a slab, in the units of the heights.
const SLAB: f64 = 28.0;

struct Proj {
    centre: [f64; 2],
    cos_y: f64,
    sin_y: f64,
    cos_t: f64,
    sin_t: f64,
    k: f64,
}

impl Proj {
    fn new(cam: &IsoCamera, centre: [f64; 2]) -> Self {
        let (y, t) = (cam.yaw.to_radians(), cam.tilt.clamp(5.0, 90.0).to_radians());
        Self {
            centre,
            cos_y: y.cos(),
            sin_y: y.sin(),
            cos_t: t.cos(),
            sin_t: t.sin(),
            k: cam.exaggeration,
        }
    }

    /// The plan turned about the centre: `(x, y)` with `y` growing toward the viewer.
    fn turn(&self, p: [f64; 2]) -> [f64; 2] {
        let (dx, dy) = (p[0] - self.centre[0], p[1] - self.centre[1]);
        [
            dx * self.cos_y - dy * self.sin_y,
            dx * self.sin_y + dy * self.cos_y,
        ]
    }

    /// The point of the plan at height `z`, in the plan-like units `Xf` turns into pixels.
    fn at(&self, p: [f64; 2], z: f64) -> [f64; 2] {
        let r = self.turn(p);
        [
            self.centre[0] + r[0],
            self.centre[1] + r[1] * self.sin_t - z * self.k * self.cos_t,
        ]
    }

    /// How near the viewer a point is: larger is nearer, which is later in the drawing.
    fn depth(&self, p: [f64; 2], z: f64) -> f64 {
        self.turn(p)[1] * self.cos_t + z * self.k * self.sin_t
    }
}

/// The polygon of a room's outline (a disc as 32 points).
fn outline(shape: &Shape) -> Vec<[f64; 2]> {
    match shape {
        Shape::Disc { c, r } => (0..32)
            .map(|i| {
                let a = std::f64::consts::TAU * f64::from(i) / 32.0;
                [c[0] + r * a.cos(), c[1] + r * a.sin()]
            })
            .collect(),
        Shape::Poly { pts } => pts.clone(),
    }
}

fn area(pts: &[[f64; 2]]) -> f64 {
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// The height of every node of the floor: its own when there is a height layer, else one storey per level.
fn node_heights(floor: &Floor) -> Vec<f64> {
    match floor.environment.as_ref().and_then(|e| e.height.as_ref()) {
        Some(h) => h.nodes.clone(),
        None => {
            let storey = floor.params.levels.storey;
            let level = tapiti_core::environment::height::node_levels(floor);
            level.iter().map(|l| *l as f64 * storey).collect()
        }
    }
}

/// What is drawn: a polygon with its depth, or a mark over everything of its depth.
enum Piece {
    Fill(Vec<Pos2>, Color32, Stroke),
    Line(Pos2, Pos2, Stroke),
    Dot(Pos2, f32, Color32),
}

struct Item {
    depth: f64,
    piece: Piece,
}

/// Everything the 2.5D view draws for a floor, far to near.
fn items(
    floor: &Floor,
    xf: &Xf,
    cam: &IsoCamera,
    overlays: Overlays,
    pal: &Palette,
) -> Vec<(f64, Piece)> {
    let node_level = tapiti_core::environment::height::node_levels(floor);
    let heights = node_heights(floor);
    let explode = cam.explode * floor.params.levels.storey;
    let level_lift = |level: usize| level as f64 * explode;
    let shown = |level: usize| {
        cam.up_to
            .filter(|_| floor.stack.is_some())
            .is_none_or(|u| level <= u)
    };
    let centre = {
        let (xs, ys) = (&floor.grid_x, &floor.grid_y);
        [
            (xs.first().copied().unwrap_or(0.0) + xs.last().copied().unwrap_or(1.0)) / 2.0,
            (ys.first().copied().unwrap_or(0.0) + ys.last().copied().unwrap_or(1.0)) / 2.0,
        ]
    };
    let proj = Proj::new(cam, centre);
    let tints = Tints::new(floor, overlays);
    let room_node: std::collections::HashMap<usize, usize> = floor
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match n {
            Node::Room { room } => Some((*room, i)),
            _ => None,
        })
        .collect();
    let screen = |p: [f64; 2], z: f64| xf.pt(proj.at(p, z));
    let mut out: Vec<Item> = Vec::new();

    // Corridors: a ribbon per segment, its height running from one end to the other.
    let half = floor.params.width.max(2.0) / 2.0;
    for e in floor
        .edges
        .iter()
        .filter(|e| !e.stair && shown(node_level[e.u]))
    {
        let lift = level_lift(node_level[e.u]);
        let total: f64 = e
            .pts
            .windows(2)
            .map(|s| tapiti_core::geom::dist(s[0], s[1]))
            .sum::<f64>()
            .max(1e-9);
        let (z0, z1) = (heights[e.u] + lift, heights[e.v] + lift);
        let mut run = 0.0;
        for s in e.pts.windows(2) {
            let len = tapiti_core::geom::dist(s[0], s[1]);
            if len <= 0.0 {
                continue;
            }
            let (ta, tb) = (run / total, (run + len) / total);
            let (za, zb) = (z0 + (z1 - z0) * ta, z0 + (z1 - z0) * tb);
            run += len;
            let n = [
                -(s[1][1] - s[0][1]) / len * half,
                (s[1][0] - s[0][0]) / len * half,
            ];
            let quad = vec![
                screen([s[0][0] + n[0], s[0][1] + n[1]], za),
                screen([s[1][0] + n[0], s[1][1] + n[1]], zb),
                screen([s[1][0] - n[0], s[1][1] - n[1]], zb),
                screen([s[0][0] - n[0], s[0][1] - n[1]], za),
            ];
            let mid = [(s[0][0] + s[1][0]) / 2.0, (s[0][1] + s[1][1]) / 2.0];
            // A hair behind a room of the same depth, so a corridor goes under the slabs it leaves.
            out.push(Item {
                depth: proj.depth(mid, (za + zb) / 2.0) - 1.0,
                piece: Piece::Fill(
                    quad,
                    pal.corridor,
                    Stroke::new(1.0, pal.ink.gamma_multiply(0.5)),
                ),
            });
        }
    }

    // Rooms: a slab with the sides that face the viewer, then the top (and the water over it).
    let water = overlays.water.then(|| {
        floor
            .environment
            .as_ref()
            .and_then(|e| e.water.as_ref().zip(e.height.as_ref()))
    });
    let light_dir = [-0.5_f64, 0.85];
    for r in floor.rooms.iter().filter(|r| r.alive && shown(r.level)) {
        let node = room_node[&r.id];
        let z = heights[node] + level_lift(r.level);
        let pts = outline(&r.shape);
        if pts.len() < 3 {
            continue;
        }
        let base = tints.fill(pal.shape(r.cls.name()), r.id);
        let alpha = if r.leaf { 0.75 } else { 0.95 };
        let ink = Stroke::new(1.0, pal.ink.gamma_multiply(0.7));
        let turned: Vec<[f64; 2]> = pts.iter().map(|p| proj.turn(*p)).collect();
        let sign = area(&turned).signum();
        let depth = proj.depth(r.center, z);
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (ta, tb) = (turned[i], turned[(i + 1) % pts.len()]);
            // The outward normal in the turned plane; it faces the viewer when it points toward larger y.
            let n = [(tb[1] - ta[1]) * sign, -(tb[0] - ta[0]) * sign];
            let len = (n[0] * n[0] + n[1] * n[1]).sqrt().max(1e-9);
            let n = [n[0] / len, n[1] / len];
            if n[1] <= 0.0 {
                continue;
            }
            let shade = 0.5 + 0.3 * ((n[0] * light_dir[0] + n[1] * light_dir[1]) + 1.0) / 2.0;
            let side = vec![
                screen(a, z),
                screen(b, z),
                screen(b, z - SLAB),
                screen(a, z - SLAB),
            ];
            out.push(Item {
                depth: depth - 0.5,
                piece: Piece::Fill(side, scale_color(base.gamma_multiply(alpha), shade), ink),
            });
        }
        let top: Vec<Pos2> = pts.iter().map(|p| screen(*p, z)).collect();
        out.push(Item {
            depth,
            piece: Piece::Fill(top.clone(), base.gamma_multiply(alpha), ink),
        });
        if let Some(Some((w, h))) = water {
            let level = w.level[node];
            if level - h.nodes[node] > 1e-9 {
                let top: Vec<Pos2> = pts
                    .iter()
                    .map(|p| screen(*p, level + level_lift(r.level)))
                    .collect();
                out.push(Item {
                    depth: depth + 0.5,
                    piece: Piece::Fill(
                        top,
                        Color32::from_rgba_unmultiplied(31, 127, 201, 140),
                        Stroke::NONE,
                    ),
                });
            }
        }
    }

    // Stairs: a column from one level to the other with a few steps, and the mark of the stair on top.
    for e in floor.edges.iter().filter(|e| e.stair) {
        let (lu, lv) = (node_level[e.u], node_level[e.v]);
        if !(shown(lu) && shown(lv)) {
            continue;
        }
        let at = e.pts[0];
        let (zu, zv) = (heights[e.u] + level_lift(lu), heights[e.v] + level_lift(lv));
        let (lo, hi) = (zu.min(zv), zu.max(zv));
        let depth = proj.depth(at, hi) + 1.0;
        let steps = 6;
        for i in 0..steps {
            let z = lo + (hi - lo) * f64::from(i) / f64::from(steps);
            let along = [at[0] + 6.0 * f64::from(i) - 15.0, at[1]];
            out.push(Item {
                depth,
                piece: Piece::Line(
                    screen(along, z),
                    screen([along[0] + 6.0, along[1]], z),
                    Stroke::new(3.0, pal.accent),
                ),
            });
        }
        out.push(Item {
            depth,
            piece: Piece::Dot(screen(at, hi), 8.0, pal.accent),
        });
    }

    // The badges of the events, on the top of the rooms they happen in.
    if overlays.events {
        if let Some(events) = floor.environment.as_ref().and_then(|e| e.events.as_ref()) {
            let mut slot = vec![0usize; floor.rooms.len()];
            for (k, kind) in events.kinds.iter().enumerate() {
                for &i in &kind.nodes {
                    let Node::Room { room } = floor.nodes[i] else {
                        continue;
                    };
                    let r: &Room = &floor.rooms[room];
                    if !shown(r.level) {
                        continue;
                    }
                    let z = heights[i] + level_lift(r.level);
                    let at = screen(r.center, z)
                        + eframe::egui::vec2(14.0 * slot[room] as f32 - 7.0, -8.0);
                    slot[room] += 1;
                    out.push(Item {
                        depth: proj.depth(r.center, z) + 2.0,
                        piece: Piece::Dot(at, 6.0, event_color(k)),
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out.into_iter().map(|i| (i.depth, i.piece)).collect()
}

fn scale_color(c: Color32, k: f64) -> Color32 {
    let k = k as f32;
    Color32::from_rgba_premultiplied(
        (f32::from(c.r()) * k) as u8,
        (f32::from(c.g()) * k) as u8,
        (f32::from(c.b()) * k) as u8,
        c.a(),
    )
}

/// Draws the floor in 2.5D.
pub fn draw(
    painter: &Painter,
    xf: &Xf,
    floor: &Floor,
    cam: &IsoCamera,
    overlays: Overlays,
    pal: &Palette,
) {
    for (_, piece) in items(floor, xf, cam, overlays, pal) {
        match piece {
            Piece::Fill(pts, fill, stroke) => {
                if pts.len() == 4 {
                    painter.add(Draw::convex_polygon(pts, fill, stroke));
                } else {
                    // A room's top may be concave: in triangles, then the outline.
                    let plain: Vec<[f64; 2]> = pts
                        .iter()
                        .map(|p| [f64::from(p.x), f64::from(p.y)])
                        .collect();
                    for t in crate::map::triangulate(&plain) {
                        painter.add(Draw::convex_polygon(
                            vec![pts[t[0]], pts[t[1]], pts[t[2]]],
                            fill,
                            Stroke::NONE,
                        ));
                    }
                    let mut ring = pts.clone();
                    ring.push(pts[0]);
                    painter.add(Draw::line(ring, stroke));
                }
            }
            Piece::Line(a, b, stroke) => {
                painter.line_segment([a, b], stroke);
            }
            Piece::Dot(at, radius, color) => {
                painter.circle_filled(at, radius, color);
                painter.circle_stroke(at, radius, Stroke::new(1.0, pal.ink));
            }
        }
    }
}

/// The box the whole oblique drawing fits in, in the plan-like units `Xf` turns into pixels, so the view starts with everything in it.
pub fn bounds(floor: &Floor, cam: &IsoCamera) -> ([f64; 2], [f64; 2]) {
    let node_level = tapiti_core::environment::height::node_levels(floor);
    let heights = node_heights(floor);
    let explode = cam.explode * floor.params.levels.storey;
    let centre = [
        (floor.grid_x.first().copied().unwrap_or(0.0)
            + floor.grid_x.last().copied().unwrap_or(1.0))
            / 2.0,
        (floor.grid_y.first().copied().unwrap_or(0.0)
            + floor.grid_y.last().copied().unwrap_or(1.0))
            / 2.0,
    ];
    let proj = Proj::new(cam, centre);
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for (i, node) in floor.nodes.iter().enumerate() {
        let Node::Room { room } = node else { continue };
        let r = &floor.rooms[*room];
        if !r.alive
            || cam
                .up_to
                .filter(|_| floor.stack.is_some())
                .is_some_and(|u| r.level > u)
        {
            continue;
        }
        let z = heights[i] + node_level[i] as f64 * explode;
        for p in outline(&r.shape) {
            for z in [z, z - SLAB] {
                let q = proj.at(p, z);
                for k in 0..2 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
            }
        }
    }
    if !lo[0].is_finite() {
        return ([0.0, 0.0], [1.0, 1.0]);
    }
    let pad = [(hi[0] - lo[0]) * 0.04, (hi[1] - lo[1]) * 0.04];
    (
        [lo[0] - pad[0], lo[1] - pad[1]],
        [hi[0] + pad[0], hi[1] + pad[1]],
    )
}

/// The room whose top lies under `pos`, the nearest first.
pub fn pick(floor: &Floor, xf: &Xf, cam: &IsoCamera, pos: Pos2) -> Option<usize> {
    let node_level = tapiti_core::environment::height::node_levels(floor);
    let heights = node_heights(floor);
    let explode = cam.explode * floor.params.levels.storey;
    let centre = [
        (floor.grid_x.first().copied().unwrap_or(0.0)
            + floor.grid_x.last().copied().unwrap_or(1.0))
            / 2.0,
        (floor.grid_y.first().copied().unwrap_or(0.0)
            + floor.grid_y.last().copied().unwrap_or(1.0))
            / 2.0,
    ];
    let proj = Proj::new(cam, centre);
    let mut best: Option<(f64, usize)> = None;
    for (i, node) in floor.nodes.iter().enumerate() {
        let Node::Room { room } = node else { continue };
        let r = &floor.rooms[*room];
        if !r.alive
            || cam
                .up_to
                .filter(|_| floor.stack.is_some())
                .is_some_and(|u| r.level > u)
        {
            continue;
        }
        let z = heights[i] + node_level[i] as f64 * explode;
        let top: Vec<[f64; 2]> = outline(&r.shape)
            .iter()
            .map(|p| {
                let s = xf.pt(proj.at(*p, z));
                [f64::from(s.x), f64::from(s.y)]
            })
            .collect();
        if inside(&top, [f64::from(pos.x), f64::from(pos.y)]) {
            let depth = proj.depth(r.center, z);
            if best.is_none_or(|(d, _)| depth > d) {
                best = Some((depth, r.id));
            }
        }
    }
    best.map(|(_, id)| id)
}

fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taller_point_is_higher_on_the_screen_and_nearer() {
        let cam = IsoCamera::default();
        let p = Proj::new(&cam, [0.0, 0.0]);
        let (low, high) = (p.at([10.0, 10.0], 0.0), p.at([10.0, 10.0], 100.0));
        assert!(high[1] < low[1], "screen y grows downward");
        assert!(p.depth([10.0, 10.0], 100.0) > p.depth([10.0, 10.0], 0.0));
    }

    #[test]
    fn looking_straight_down_is_the_plan_turned() {
        let cam = IsoCamera {
            yaw: 0.0,
            tilt: 90.0,
            ..IsoCamera::default()
        };
        let p = Proj::new(&cam, [0.0, 0.0]);
        let at = p.at([30.0, 40.0], 0.0);
        assert!((at[0] - 30.0).abs() < 1e-9 && (at[1] - 40.0).abs() < 1e-9);
        // Height does not move a point seen from straight above.
        assert!((p.at([30.0, 40.0], 500.0)[1] - 40.0).abs() < 1e-6);
    }

    #[test]
    fn a_square_is_outside_in_either_winding() {
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(inside(&square, [5.0, 5.0]) && !inside(&square, [15.0, 5.0]));
        let reversed: Vec<[f64; 2]> = square.iter().rev().copied().collect();
        assert!(inside(&reversed, [5.0, 5.0]));
        assert!(area(&square) * area(&reversed) < 0.0);
    }
}
