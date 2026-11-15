//! Plan a ring of rooms round an empty court before ordinary growth fills the remaining space.

use super::*;

pub(super) type FitRoom<'a> = dyn Fn(&mut Gen<'_>, Bx, i32) -> Option<(ShapeClass, Shape)> + 'a;
pub(super) type MakeRoom<'a> = dyn Fn(usize, ShapeClass, Shape, &Bx) -> Room + 'a;

/// The part of the ring ellipse (as a fraction of its radii) kept empty: no room, corridor or chamber may enter it.
const COURT: f64 = 0.5;

/// Whether `q` is inside the court ellipse of centre `c` and radii `r`.
fn in_court(q: V, c: V, r: V) -> bool {
    let (x, y) = (
        (q[0] - c[0]) / (COURT * r[0]),
        (q[1] - c[1]) / (COURT * r[1]),
    );
    x * x + y * y < 1.0
}

impl<'a> Gen<'a> {
    /// Whether a corridor path crosses the court.
    pub(super) fn crosses_court(&self, pts: &[V]) -> bool {
        let Some((c, r)) = self.court else {
            return false;
        };
        pts.windows(2).any(|s| {
            let n = (dist(s[0], s[1]) / 16.0).ceil().max(1.0) as usize;
            (0..=n).any(|i| {
                let t = i as f64 / n as f64;
                in_court(
                    [
                        s[0][0] + (s[1][0] - s[0][0]) * t,
                        s[0][1] + (s[1][1] - s[0][1]) * t,
                    ],
                    c,
                    r,
                )
            })
        })
    }

    /// Whether a box reaches into the court.
    pub(super) fn box_in_court(&self, b: &Bx) -> bool {
        let Some((c, r)) = self.court else {
            return false;
        };
        let mid = [(b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0];
        [[b.x0, b.y0], [b.x1, b.y0], [b.x0, b.y1], [b.x1, b.y1], mid]
            .into_iter()
            .any(|q| in_court(q, c, r))
    }
}

/// Places `p.ring_rooms()` rooms on the ring ellipse from room 0 (at the north, the entrance), each joined to the one before, then
/// closes the ring from the last room to room 0. Returns false when a room or the closing corridor does not fit.
pub(super) fn build(
    g: &mut Gen<'_>,
    world: Bx,
    fit_room: &FitRoom<'_>,
    make_room: &MakeRoom<'_>,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) -> bool {
    let p = g.p;
    let (centre, radii) = p.ring_ellipse();
    g.court = Some((centre, radii));
    let count = p.ring_rooms();
    let (bw, bh) = (p.cell_w - 2.0 * p.gap, p.cell_h - 2.0 * p.gap);
    g.ring.push(0);
    for k in 1..count {
        // Clockwise from the north: the angle is measured from the top, so room 0 stands at the top of the ellipse.
        let theta = std::f64::consts::TAU * k as f64 / count as f64;
        let target = [
            centre[0] + radii[0] * theta.det_sin(),
            centre[1] - radii[1] * theta.det_cos(),
        ];
        let parent = g.ring[k - 1];
        let mut placed = false;
        for _ in 0..12 {
            let (jx, jy) = (g.rng.next() - 0.5, g.rng.next() - 0.5);
            let (cx, cy) = (target[0] + jx * 0.4 * bw, target[1] + jy * 0.4 * bh);
            let bx = Bx {
                x0: cx - bw / 2.0,
                y0: cy - bh / 2.0,
                x1: cx + bw / 2.0,
                y1: cy + bh / 2.0,
            };
            if bx.x0 < world.x0 || bx.y0 < world.y0 || bx.x1 > world.x1 || bx.y1 > world.y1 {
                continue;
            }
            let Some((c, shape)) = fit_room(g, bx, 1) else {
                continue;
            };
            let room = make_room(g.rooms.len(), c, shape, &bx);
            let id = room.id;
            let crowded = g
                .room_grid
                .query_bounds(room.bounds.lo, room.bounds.hi, 2.0 * p.gap)
                .into_iter()
                .any(|o| shape_within(&room.shape, &g.rooms[o].shape, 2.0 * p.gap));
            if crowded || g.box_in_court(&bx) {
                continue;
            }
            g.rooms.push(room);
            // The link to the last room is made when the ring closes, so this one must also reach the first when it is the last.
            let link = candidates_for_indexed(p, &g.rooms, parent, id, 4, &g.room_grid)
                .into_iter()
                .filter(|c| g.fits_now(c))
                .min_by(|a, b| a.cost.total_cmp(&b.cost));
            let Some(link) = link else {
                g.rooms.pop();
                continue;
            };
            g.room_grid
                .insert_bounds(id, g.rooms[id].bounds.lo, g.rooms[id].bounds.hi);
            g.nodes.push(Node::Room { room: id });
            g.node_of_room.push(id);
            report_progress(progress, || {
                GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
            });
            let edge = g.add_edge(
                g.node_of_room[parent],
                g.node_of_room[id],
                link.pts,
                [Some(link.ports[0]), Some(link.ports[1])],
            );
            report_progress(progress, || GenerationEvent::EdgeUpdated {
                id: edge,
                pts: g.edges[edge].pts.clone(),
            });
            g.ring.push(id);
            placed = true;
            break;
        }
        if !placed {
            g.log.push(format!("ring stopped at room {k} of {count}"));
            return false;
        }
    }
    let last = g.ring[count - 1];
    let close = candidates_for_indexed(p, &g.rooms, last, 0, 4, &g.room_grid)
        .into_iter()
        .filter(|c| g.fits_now(c))
        .min_by(|a, b| a.cost.total_cmp(&b.cost));
    let Some(close) = close else {
        g.log.push("ring could not close".to_string());
        return false;
    };
    let edge = g.add_edge(
        g.node_of_room[last],
        g.node_of_room[0],
        close.pts,
        [Some(close.ports[0]), Some(close.ports[1])],
    );
    report_progress(progress, || GenerationEvent::EdgeUpdated {
        id: edge,
        pts: g.edges[edge].pts.clone(),
    });
    true
}
