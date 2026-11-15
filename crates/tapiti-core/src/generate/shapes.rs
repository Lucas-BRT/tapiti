//! Room shapes that fit a grid block.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Bx {
    pub(super) x0: f64,
    pub(super) y0: f64,
    pub(super) x1: f64,
    pub(super) y1: f64,
}

pub(super) fn poly(pts: Vec<V>) -> Shape {
    Shape::Poly { pts }
}

pub(super) fn rotate_poly_in_box(shape: Shape, b: Bx, rng: &mut dyn Rand, m: i32) -> Shape {
    let Shape::Poly { pts } = shape else {
        return shape;
    };
    let steps = if m == 8 { 8 } else { 4 };
    let first = (rng.next() * steps as f64).floor() as i32;
    let center = centroid(&Shape::Poly { pts: pts.clone() });
    for offset in 0..steps {
        let k = (first + offset) % steps;
        let a = 2.0 * PI * k as f64 / steps as f64;
        let (sin, cos) = a.sin_cos();
        let rotated: Vec<V> = pts
            .iter()
            .map(|q| {
                let d = sub(*q, center);
                [
                    center[0] + cos * d[0] - sin * d[1],
                    center[1] + sin * d[0] + cos * d[1],
                ]
            })
            .collect();
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for q in &rotated {
            lo = [lo[0].min(q[0]), lo[1].min(q[1])];
            hi = [hi[0].max(q[0]), hi[1].max(q[1])];
        }
        let span = [hi[0] - lo[0], hi[1] - lo[1]];
        let w = b.x1 - b.x0;
        let h = b.y1 - b.y0;
        if span[0] <= w && span[1] <= h {
            let dx = b.x0 - lo[0] + rng.next() * (w - span[0]);
            let dy = b.y0 - lo[1] + rng.next() * (h - span[1]);
            return poly(
                rotated
                    .into_iter()
                    .map(|q| [q[0] + dx, q[1] + dy])
                    .collect(),
            );
        }
    }
    poly(pts)
}

/// fit_κ (§2.2): a shape of class `cls` inside `b`, its proportions drawn from `rng`. None when the class doesn't fit (the caller falls back to a rectangle).
pub(super) fn fit(
    cls: ShapeClass,
    b: Bx,
    rng: &mut dyn Rand,
    p: &Params,
    cells: i32,
) -> Option<Shape> {
    let w_ = b.x1 - b.x0;
    let h_ = b.y1 - b.y0;
    let iota = p.width / 2.0 + p.clear;
    let min_side = 2.0 * iota + p.width;
    if w_ < min_side || h_ < min_side {
        return None;
    }
    let shape = match cls {
        ShapeClass::Rect => {
            let w = rng.range(
                if cells > 1 {
                    w_ * 0.7
                } else {
                    w_.min(min_side.max(w_ * 0.55))
                },
                w_,
            );
            let h = rng.range(
                if cells > 1 {
                    h_ * 0.7
                } else {
                    h_.min(min_side.max(h_ * 0.55))
                },
                h_,
            );
            let x = b.x0 + rng.next() * (w_ - w);
            let y = b.y0 + rng.next() * (h_ - h);
            Some(poly(vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]))
        }
        ShapeClass::Disc => {
            let r = (w_.min(h_) / 2.0) * rng.range(0.8, 1.0);
            if r < p.width {
                return None;
            }
            let cx = b.x0 + r + rng.next() * (w_ - 2.0 * r);
            let cy = b.y0 + r + rng.next() * (h_ - 2.0 * r);
            Some(Shape::Disc { c: [cx, cy], r })
        }
        ShapeClass::Oct => {
            // Vertices at 22.5° + k·45°: the edges face the 8 directions of the alphabet.
            let half = (w_.min(h_) / 2.0) * rng.range(0.8, 1.0);
            let rho = half / (PI / 8.0).det_cos();
            // Every edge must hold a door: |e| = 2ρ·sin(22.5°) ≥ 2ι.
            if 2.0 * rho * (PI / 8.0).det_sin() < 2.0 * iota + 4.0 {
                return None;
            }
            let cx = b.x0 + half + rng.next() * (w_ - 2.0 * half);
            let cy = b.y0 + half + rng.next() * (h_ - 2.0 * half);
            let pts = (0..8)
                .map(|i| {
                    let a = PI / 8.0 + i as f64 * PI / 4.0;
                    add([cx, cy], mul([a.det_cos(), a.det_sin()], rho))
                })
                .collect();
            Some(poly(pts))
        }
        ShapeClass::L => {
            let w = rng.range(w_ * 0.75, w_);
            let h = rng.range(h_ * 0.75, h_);
            let a = w * rng.range(0.35, 0.55);
            let n = h * rng.range(0.35, 0.55);
            let x = b.x0 + rng.next() * (w_ - w);
            let y = b.y0 + rng.next() * (h_ - h);
            // The notch at the top-left corner, then mirrored to the corner drawn.
            let local: [V; 6] = [[a, 0.0], [w, 0.0], [w, h], [0.0, h], [0.0, n], [a, n]];
            let corner = (rng.next() * 4.0).floor() as i32;
            let fx = corner == 1 || corner == 2;
            let fy = corner == 2 || corner == 3;
            Some(poly(
                local
                    .iter()
                    .map(|&[px, py]| {
                        [
                            x + if fx { w - px } else { px },
                            y + if fy { h - py } else { py },
                        ]
                    })
                    .collect(),
            ))
        }
        ShapeClass::Star => {
            if cells < p.star_cells {
                return None;
            }
            // The draw that used to pick 4 or 8 points stays, so the rest of the stream is the same as before.
            if p.m == 8 {
                rng.next();
            }
            // A four-pointed star with its points on the diagonals and a flat notch on each axis. Every door sits on a notch (a straight
            // stretch of `2 * iota + 8`, facing an axis, where the corridor leaves between two points), so the points themselves can be
            // sharp: nothing needs a flat tip any more.
            let tip = (w_.min(h_) / 2.0) * rng.range(0.9, 1.0);
            let sharp = rng.range(0.45, 0.55) / 0.5;
            let half = iota + 4.0;
            if tip < half + 22.0 {
                return None;
            }
            // The notch floor lies between the half-length of the notch and the tip, nearer the notch the sharper the star.
            let floor = half + (tip - half) * 0.35 * sharp;
            let mut local: Vec<V> = Vec::new();
            for k in 0..4 {
                let (c, s) = (
                    (PI / 2.0 * k as f64).det_cos(),
                    (PI / 2.0 * k as f64).det_sin(),
                );
                let turn = |q: V| [q[0] * c - q[1] * s, q[0] * s + q[1] * c];
                // East notch (top end first), then the tip on the next diagonal.
                local.push(turn([floor, -half]));
                local.push(turn([floor, half]));
                local.push(turn([tip, tip]));
            }
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for q in &local {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let span = [hi[0] - lo[0], hi[1] - lo[1]];
            if span[0] > w_ || span[1] > h_ {
                return None;
            }
            let cx = b.x0 - lo[0] + rng.next() * (w_ - span[0]);
            let cy = b.y0 - lo[1] + rng.next() * (h_ - span[1]);
            let pts = local.iter().map(|q| [cx + q[0], cy + q[1]]).collect();
            Some(poly(pts))
        }
    }?;
    Some(if p.rotate_rooms {
        rotate_poly_in_box(shape, b, rng, p.m)
    } else {
        shape
    })
}
