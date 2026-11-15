//! Walking the floor on foot: where a person standing at a point of the plan has ground under their feet, so a view from inside can
//! move through doors and along ramps, stay out of the walls and climb a stair. It reads the floor with the same heights and options as
//! the mesh, so what is drawn is what is walked on.

use crate::mesh::{node_heights, outline, MeshOptions};
use tapiti_core::generate::{Floor, Node};
use tapiti_core::geom::{dist, V};

/// The most a step may climb, and the most a fall may drop, in the units of the floor.
const STEP: f32 = 16.0;
const DROP: f32 = 80.0;
/// The height of the eyes above the feet (a corridor is 44 units wide, about 2.5 m).
pub const EYE: f32 = 29.0;

struct RoomGround {
    poly: Vec<V>,
    y: f32,
}

struct CorridorGround {
    pts: Vec<V>,
    /// Length of the polyline up to each point.
    run: Vec<f64>,
    y0: f32,
    y1: f32,
    half: f64,
}

struct StairGround {
    at: V,
    low: f32,
    high: f32,
}

/// The ground of a floor.
pub struct WalkMap {
    rooms: Vec<RoomGround>,
    corridors: Vec<CorridorGround>,
    stairs: Vec<StairGround>,
    start: Option<([f32; 3], f32)>,
}

fn inside(poly: &[V], p: V) -> bool {
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

impl WalkMap {
    pub fn new(floor: &Floor, o: &MeshOptions) -> Self {
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
        let mut rooms = Vec::new();
        let mut start = None;
        for (i, node) in floor.nodes.iter().enumerate() {
            let Node::Room { room } = node else { continue };
            let r = &floor.rooms[*room];
            if !r.alive || !shown(r.level) {
                continue;
            }
            let y = y_of(i);
            if floor.entrance == Some(r.id) {
                // Facing the first corridor that leaves the entrance, so the walk starts looking at a way out.
                let toward = floor
                    .edges
                    .iter()
                    .find(|e| e.u == i || e.v == i)
                    .and_then(|e| {
                        if e.u == i {
                            e.pts.get(1)
                        } else {
                            e.pts.len().checked_sub(2).and_then(|k| e.pts.get(k))
                        }
                        .copied()
                    });
                let yaw = toward.map_or(0.0, |t| {
                    (t[0] - r.center[0]).atan2(t[1] - r.center[1]) as f32
                });
                start = Some(([r.center[0] as f32, y, r.center[1] as f32], yaw));
            }
            rooms.push(RoomGround {
                poly: outline(&r.shape),
                y,
            });
        }
        let half = floor.params.width.max(2.0) / 2.0;
        let corridors = floor
            .edges
            .iter()
            .filter(|e| !e.stair && shown(node_level[e.u]))
            .map(|e| {
                let mut run = vec![0.0];
                for p in e.pts.windows(2) {
                    run.push(run.last().unwrap() + dist(p[0], p[1]));
                }
                CorridorGround {
                    pts: e.pts.clone(),
                    run,
                    y0: y_of(e.u),
                    y1: y_of(e.v),
                    half,
                }
            })
            .collect();
        let stairs = floor
            .edges
            .iter()
            .filter(|e| e.stair && shown(node_level[e.u]) && shown(node_level[e.v]))
            .map(|e| {
                let (a, b) = (y_of(e.u), y_of(e.v));
                StairGround {
                    at: e.pts[0],
                    low: a.min(b),
                    high: a.max(b),
                }
            })
            .collect();
        Self {
            rooms,
            corridors,
            stairs,
            start,
        }
    }

    /// Where to start: the centre of the entrance room (feet) and the way to face.
    pub fn start(&self) -> Option<([f32; 3], f32)> {
        self.start
    }

    /// The height of the ground at `p` for someone whose feet are at `from`: the highest ground within a step above them, or the
    /// nearest below within a drop. `None` where there is only wall (or too far a fall).
    pub fn ground(&self, p: V, from: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        let mut offer = |y: f32| {
            if y <= from + STEP && y >= from - DROP && best.is_none_or(|b| y > b) {
                best = Some(y);
            }
        };
        for r in &self.rooms {
            if inside(&r.poly, p) {
                offer(r.y);
            }
        }
        for c in &self.corridors {
            let total = c.run.last().copied().unwrap_or(0.0).max(1e-9);
            for (k, s) in c.pts.windows(2).enumerate() {
                let len = dist(s[0], s[1]);
                if len <= 0.0 {
                    continue;
                }
                let t = (((p[0] - s[0][0]) * (s[1][0] - s[0][0])
                    + (p[1] - s[0][1]) * (s[1][1] - s[0][1]))
                    / (len * len))
                    .clamp(0.0, 1.0);
                let nearest = [
                    s[0][0] + (s[1][0] - s[0][0]) * t,
                    s[0][1] + (s[1][1] - s[0][1]) * t,
                ];
                if dist(p, nearest) <= c.half {
                    let along = (c.run[k] + len * t) / total;
                    offer(c.y0 + (c.y1 - c.y0) * along as f32);
                }
            }
        }
        best
    }

    /// Whether a person of `radius` can stand at `p`: the ground is under every point round them.
    fn stands(&self, p: V, from: f32, radius: f64) -> Option<f32> {
        let y = self.ground(p, from)?;
        for k in 0..8 {
            let a = std::f64::consts::TAU * f64::from(k) / 8.0;
            self.ground(
                [p[0] + radius * libm::cos(a), p[1] + radius * libm::sin(a)],
                from,
            )?;
        }
        Some(y)
    }

    /// Moves the feet at `pos` by `(dx, dz)` on the plan, as far as the ground lets them: all of it, else along a wall (one axis), else
    /// not at all. The height follows the ground.
    pub fn step(&self, pos: &mut [f32; 3], dx: f32, dz: f32, radius: f64) {
        for (mx, mz) in [(dx, dz), (dx, 0.0), (0.0, dz)] {
            if mx == 0.0 && mz == 0.0 {
                continue;
            }
            let p = [f64::from(pos[0] + mx), f64::from(pos[2] + mz)];
            if let Some(y) = self.stands(p, pos[1], radius) {
                *pos = [p[0] as f32, y, p[1] as f32];
                return;
            }
        }
    }

    /// The height to climb or go down to, when the feet at `pos` are at a stair (within `reach` of it, on its lower or upper floor).
    pub fn stair_at(&self, pos: [f32; 3], reach: f64) -> Option<f32> {
        self.stairs.iter().find_map(|s| {
            let near = dist([f64::from(pos[0]), f64::from(pos[2])], s.at) <= reach;
            let (to_low, to_high) = ((pos[1] - s.low).abs(), (pos[1] - s.high).abs());
            (near && to_low.min(to_high) <= STEP * 2.0).then_some(if to_low < to_high {
                s.high
            } else {
                s.low
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{
        generate::generate,
        params::{params_of_size, Layout, Size},
    };

    fn rect_floor() -> Floor {
        let mut p = params_of_size(Size::Medium);
        p.layout = Layout::Growth;
        generate(2, &p)
    }

    #[test]
    fn the_walk_starts_in_the_entrance_with_ground_and_wall_all_round() {
        let f = rect_floor();
        let map = WalkMap::new(&f, &MeshOptions::default());
        let (pos, _) = map.start().expect("an entrance");
        assert!(map
            .ground([f64::from(pos[0]), f64::from(pos[2])], pos[1])
            .is_some());
        // Far outside the world there is nothing, and a step that way does not move.
        let mut at = pos;
        assert!(map.ground([-1.0e5, -1.0e5], pos[1]).is_none());
        for _ in 0..2000 {
            map.step(&mut at, -4.0, 0.0, 6.0);
        }
        assert!(at[0] > -1.0e4, "walked out of the floor");
    }

    #[test]
    fn a_walker_goes_out_of_the_entrance_by_a_door_and_along_the_corridor_to_the_next_room() {
        let f = rect_floor();
        let map = WalkMap::new(&f, &MeshOptions::default());
        let entrance = f.entrance.unwrap();
        let node = f
            .nodes
            .iter()
            .position(|n| matches!(n, Node::Room { room } if *room == entrance))
            .unwrap();
        let e = f
            .edges
            .iter()
            .find(|e| e.u == node || e.v == node)
            .expect("a corridor from the entrance");
        // The corridor from the entrance's door to the other end, in order.
        let mut route: Vec<V> = e.pts.clone();
        if e.v == node {
            route.reverse();
        }
        let (mut pos, _) = map.start().unwrap();
        let mut arrived = true;
        for target in route.iter().chain(std::iter::once(
            &f.rooms[match f.nodes[if e.u == node { e.v } else { e.u }] {
                Node::Room { room } => room,
                _ => entrance,
            }]
            .center,
        )) {
            let mut moved = 0;
            while dist([f64::from(pos[0]), f64::from(pos[2])], *target) > 4.0 && moved < 4000 {
                let d = [target[0] as f32 - pos[0], target[1] as f32 - pos[2]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
                let before = pos;
                map.step(&mut pos, d[0] / l * 2.0, d[1] / l * 2.0, 6.0);
                if pos == before {
                    break;
                }
                moved += 1;
            }
            arrived &= dist([f64::from(pos[0]), f64::from(pos[2])], *target) <= 6.0;
        }
        assert!(arrived, "stuck at {pos:?}");
    }

    #[test]
    fn a_stair_takes_a_walker_to_the_other_level() {
        let mut p = params_of_size(Size::Medium);
        p.layout = Layout::Growth;
        p.levels.count = 2;
        p.levels.stairs = 2;
        let f = (1..=10)
            .map(|s| generate(s, &p))
            .find(|f| f.edges.iter().any(|e| e.stair))
            .expect("a stacked floor with a stair");
        let map = WalkMap::new(&f, &MeshOptions::default());
        let e = f.edges.iter().find(|e| e.stair).unwrap();
        let low = f
            .rooms
            .iter()
            .find(|r| r.level == 0 && r.alive && tapiti_core::geom::dist(r.center, e.pts[0]) < 1e9)
            .is_some();
        assert!(low);
        let level0_y = 0.0;
        let pos = [e.pts[0][0] as f32, level0_y, e.pts[0][1] as f32];
        let up = map.stair_at(pos, 20.0).expect("a stair at its own point");
        assert!(up > level0_y, "the stair does not climb");
        let there = [pos[0], up, pos[2]];
        assert_eq!(map.stair_at(there, 20.0), Some(level0_y), "and back down");
        assert!(map
            .stair_at([pos[0] + 500.0, level0_y, pos[2]], 20.0)
            .is_none());
    }
}
