//! Binary space partition: the world is cut into leaves, one room stands in each, and sibling subtrees are joined by a corridor.

use super::ring::{FitRoom, MakeRoom};
use super::*;

struct Cut {
    rect: Bx,
    kids: Option<(usize, usize)>,
}

/// The leaves of `target` cuts: the largest leaf that still holds two rooms is cut across its longer side, off-centre, so the
/// proportions differ from leaf to leaf. Returns the tree, whose last leaves may be fewer than asked when nothing can be cut.
fn cut(g: &mut Gen<'_>, world: Bx, target: usize) -> Vec<Cut> {
    let p = g.p;
    // A leaf holds a room of the smallest box plus the gaps that keep it from the next room.
    let side = p.cell_w.min(p.cell_h).max(1.0);
    let mut tree = vec![Cut {
        rect: world,
        kids: None,
    }];
    let mut leaves = vec![0usize];
    // Zones joined by single corridors: the first cuts are the zone boundaries, so each boundary is crossed by one sibling join.
    if p.bottleneck_on() {
        let wide = p.cols.max(1) as f64 * p.cell_w >= p.rows.max(1) as f64 * p.cell_h;
        let mut rest = 0usize;
        for k in 1..p.zone_count() {
            let (z, r) = (p.zone_strip(k), tree[rest].rect);
            let (a, b) = if wide {
                (Bx { x1: z[0], ..r }, Bx { x0: z[0], ..r })
            } else {
                (Bx { y1: z[1], ..r }, Bx { y0: z[1], ..r })
            };
            let (ia, ib) = (tree.len(), tree.len() + 1);
            tree.push(Cut {
                rect: a,
                kids: None,
            });
            tree.push(Cut {
                rect: b,
                kids: None,
            });
            tree[rest].kids = Some((ia, ib));
            leaves.retain(|l| *l != rest);
            leaves.push(ia);
            leaves.push(ib);
            rest = ib;
        }
    }
    let area = |b: &Bx| (b.x1 - b.x0) * (b.y1 - b.y0);
    while leaves.len() < target {
        let mut order: Vec<usize> = (0..leaves.len()).collect();
        order.sort_by(|a, b| area(&tree[leaves[*b]].rect).total_cmp(&area(&tree[leaves[*a]].rect)));
        let Some(&k) = order.iter().find(|&&k| {
            let r = &tree[leaves[k]].rect;
            (r.x1 - r.x0).max(r.y1 - r.y0) >= 2.0 * side
        }) else {
            break;
        };
        let id = leaves[k];
        let r = tree[id].rect;
        let ratio = 0.38 + 0.24 * g.rng.next();
        let (a, b) = if r.x1 - r.x0 >= r.y1 - r.y0 {
            let x = r.x0 + (r.x1 - r.x0) * ratio;
            (Bx { x1: x, ..r }, Bx { x0: x, ..r })
        } else {
            let y = r.y0 + (r.y1 - r.y0) * ratio;
            (Bx { y1: y, ..r }, Bx { y0: y, ..r })
        };
        // The cut must leave both halves able to hold a room.
        let fits = |b: &Bx| b.x1 - b.x0 >= side && b.y1 - b.y0 >= side;
        let (a, b) = if fits(&a) && fits(&b) {
            (a, b)
        } else if r.x1 - r.x0 >= r.y1 - r.y0 {
            let x = (r.x0 + r.x1) / 2.0;
            (Bx { x1: x, ..r }, Bx { x0: x, ..r })
        } else {
            let y = (r.y0 + r.y1) / 2.0;
            (Bx { y1: y, ..r }, Bx { y0: y, ..r })
        };
        let (ia, ib) = (tree.len(), tree.len() + 1);
        tree.push(Cut {
            rect: a,
            kids: None,
        });
        tree.push(Cut {
            rect: b,
            kids: None,
        });
        tree[id].kids = Some((ia, ib));
        leaves.swap_remove(k);
        leaves.push(ia);
        leaves.push(ib);
    }
    tree
}

/// Builds the rooms and the corridors between sibling subtrees. Returns false when a pair of subtrees cannot be joined.
pub(super) fn build(
    g: &mut Gen<'_>,
    world: Bx,
    target: usize,
    fit_room: &FitRoom<'_>,
    make_room: &MakeRoom<'_>,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) -> bool {
    let p = g.p;
    let tree = cut(g, world, target.max(2 * p.zone_count()));
    let mut leaves: Vec<usize> = (0..tree.len())
        .filter(|&i| tree[i].kids.is_none())
        .collect();
    // The first room is the entrance: the leaf farthest west.
    leaves.sort_by(|a, b| {
        let (ra, rb) = (&tree[*a].rect, &tree[*b].rect);
        ra.x0.total_cmp(&rb.x0).then(ra.y0.total_cmp(&rb.y0))
    });
    let (cw, ch) = (p.cell_w.max(1.0), p.cell_h.max(1.0));
    // The smallest box a room is drawn in (a cell less its two gaps).
    let small = (cw.min(ch) - 2.0 * p.gap).max(1.0);
    let mut room_of = vec![usize::MAX; tree.len()];
    for &leaf in &leaves {
        if g.stopped() {
            return false;
        }
        // A gap in from every side of its leaf, so rooms of neighbouring leaves stand two gaps apart.
        let r = Bx {
            x0: tree[leaf].rect.x0 + p.gap,
            y0: tree[leaf].rect.y0 + p.gap,
            x1: tree[leaf].rect.x1 - p.gap,
            y1: tree[leaf].rect.y1 - p.gap,
        };
        // A room fills 65% to 100% of each side of its leaf, up to three cells, at a random place inside it.
        let side = |lo: f64, hi: f64, g: &mut Gen<'_>| {
            let room = ((hi - lo) * (0.65 + 0.35 * g.rng.next()))
                .max(small.min(hi - lo))
                .min(3.0 * cw.max(ch));
            let at = lo + (hi - lo - room) * g.rng.next();
            (at, at + room)
        };
        let (x0, x1) = side(r.x0, r.x1, g);
        let (y0, y1) = side(r.y0, r.y1, g);
        let bx = Bx { x0, y0, x1, y1 };
        let cells = (((x1 - x0) / cw) * ((y1 - y0) / ch)).round().max(1.0) as i32;
        let Some((c, shape)) = fit_room(g, bx, cells) else {
            g.log.push("bsp: no room fits a leaf".to_string());
            return false;
        };
        let room = make_room(g.rooms.len(), c, shape, &bx);
        let id = room.id;
        g.room_grid
            .insert_bounds(id, room.bounds.lo, room.bounds.hi);
        g.rooms.push(room);
        g.nodes.push(Node::Room { room: id });
        g.node_of_room.push(id);
        report_progress(progress, || {
            GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
        });
        room_of[leaf] = id;
    }
    // Siblings are joined bottom-up: the closest pair of rooms, one from each side, that a corridor reaches.
    fn join(
        g: &mut Gen<'_>,
        tree: &[Cut],
        room_of: &[usize],
        node: usize,
        make_room: &MakeRoom<'_>,
        progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
    ) -> Option<Vec<usize>> {
        let Some((a, b)) = tree[node].kids else {
            return Some(vec![room_of[node]]);
        };
        if g.stopped() {
            return None;
        }
        let left = join(g, tree, room_of, a, make_room, progress)?;
        let right = join(g, tree, room_of, b, make_room, progress)?;
        let mut pairs: Vec<(f64, usize, usize)> = left
            .iter()
            .flat_map(|&x| right.iter().map(move |&y| (x, y)))
            .map(|(x, y)| (dist(g.rooms[x].center, g.rooms[y].center), x, y))
            .collect();
        pairs.sort_by(|u, v| u.0.total_cmp(&v.0));
        // Two lone rooms no corridor joins (round rooms offset from each other have only their four doors) become rectangles,
        // whose doors line up along a whole side, and are tried once more.
        let lone = left.len() == 1
            && right.len() == 1
            && g.rooms[left[0]].used.is_empty()
            && g.rooms[right[0]].used.is_empty();
        for pass in 0..2 {
            if pass == 1 {
                if !lone {
                    break;
                }
                for &id in &[left[0], right[0]] {
                    let r = g.rooms[id].rect;
                    let bx = Bx {
                        x0: r.x0,
                        y0: r.y0,
                        x1: r.x1,
                        y1: r.y1,
                    };
                    if let Some(shape) = fit(ShapeClass::Rect, bx, &mut g.rng, g.p, 1) {
                        g.rooms[id] = make_room(id, ShapeClass::Rect, shape, &bx);
                        g.room_grid
                            .insert_bounds(id, g.rooms[id].bounds.lo, g.rooms[id].bounds.hi);
                    }
                }
            }
            for (_, x, y) in pairs.iter().copied().take(120) {
                let best = candidates_for_indexed(g.p, &g.rooms, x, y, 8, &g.room_grid)
                    .into_iter()
                    .filter(|c| g.fits_now(c))
                    .min_by(|u, v| u.cost.total_cmp(&v.cost));
                let Some(c) = best else { continue };
                let edge = g.add_edge(
                    g.node_of_room[x],
                    g.node_of_room[y],
                    c.pts,
                    [Some(c.ports[0]), Some(c.ports[1])],
                );
                report_progress(progress, || GenerationEvent::EdgeUpdated {
                    id: edge,
                    pts: g.edges[edge].pts.clone(),
                });
                return Some(left.into_iter().chain(right).collect());
            }
        }
        None
    }
    if join(g, &tree, &room_of, 0, make_room, progress).is_none() {
        g.log
            .push("bsp: two halves could not be joined".to_string());
        return false;
    }
    true
}
