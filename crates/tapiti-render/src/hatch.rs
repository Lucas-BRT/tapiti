//! Shape hatches shared by SVG exports and the native map. Marks are clipped to the room, so inline SVGs need no global pattern ids.

use tapiti_core::geom::{bounds_of, dot, Shape, V};

#[derive(Clone, Copy, Debug)]
pub enum HatchMark {
    Dot(V),
    Line(V, V),
}

/// Disc dots, octagon diagonals, L horizontals and star crosses. Rectangles stay plain.
/// `spacing` is in the shape's coordinates; a screen renderer can scale it to keep the marks legible when zooming.
pub fn hatch(shape: &Shape, class: &str, spacing: f64) -> Vec<HatchMark> {
    let mut marks = Vec::new();
    if !spacing.is_finite() || spacing <= 0.0 {
        return marks;
    }
    if let ("disc", Shape::Disc { c, r }) = (class, shape) {
        let (lo, hi) = bounds_of(shape);
        for y in (lo[1] / spacing).ceil() as i64..=(hi[1] / spacing).floor() as i64 {
            for x in (lo[0] / spacing).ceil() as i64..=(hi[0] / spacing).floor() as i64 {
                let p = [x as f64 * spacing, y as f64 * spacing];
                if (p[0] - c[0]).hypot(p[1] - c[1]) + spacing * 0.12 < *r {
                    marks.push(HatchMark::Dot(p));
                }
            }
        }
        return marks;
    }
    let Shape::Poly { pts } = shape else {
        return marks;
    };
    let normals: &[V] = match class {
        "oct" => &[[1.0, 1.0]],
        "L" => &[[0.0, 1.0]],
        "star" => &[[0.0, 1.0], [1.0, 0.0]],
        _ => return marks,
    };
    for &normal in normals {
        let lo = pts
            .iter()
            .map(|p| dot(*p, normal))
            .fold(f64::INFINITY, f64::min);
        let hi = pts
            .iter()
            .map(|p| dot(*p, normal))
            .fold(f64::NEG_INFINITY, f64::max);
        for row in (lo / spacing).ceil() as i64..=(hi / spacing).floor() as i64 {
            let at = row as f64 * spacing;
            let mut hits = Vec::new();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                let (da, db) = (dot(a, normal), dot(b, normal));
                // Half-open edges count each vertex once, including at a concave notch.
                if (da <= at && db > at) || (db <= at && da > at) {
                    let t = (at - da) / (db - da);
                    hits.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                }
            }
            let along = [-normal[1], normal[0]];
            hits.sort_by(|a, b| dot(*a, along).total_cmp(&dot(*b, along)));
            for pair in hits.as_chunks::<2>().0 {
                if (pair[0][0] - pair[1][0]).hypot(pair[0][1] - pair[1][1]) > 1e-6 {
                    marks.push(HatchMark::Line(pair[0], pair[1]));
                }
            }
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::geom::inside_poly;

    #[test]
    fn hatches_stay_inside_concave_rooms_in_either_winding() {
        let l = vec![
            [0.0, 0.0],
            [40.0, 0.0],
            [40.0, 20.0],
            [20.0, 20.0],
            [20.0, 40.0],
            [0.0, 40.0],
        ];
        let star = vec![
            [0.0, -40.0],
            [10.0, -10.0],
            [40.0, 0.0],
            [10.0, 10.0],
            [0.0, 40.0],
            [-10.0, 10.0],
            [-40.0, 0.0],
            [-10.0, -10.0],
        ];
        for (class, pts) in [("L", l), ("star", star)] {
            for reversed in [false, true] {
                let mut pts = pts.clone();
                if reversed {
                    pts.reverse();
                }
                let marks = hatch(&Shape::Poly { pts: pts.clone() }, class, 9.0);
                assert!(!marks.is_empty());
                for mark in marks {
                    let HatchMark::Line(a, b) = mark else {
                        panic!("expected lines")
                    };
                    for t in [0.01, 0.5, 0.99] {
                        let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                        assert!(inside_poly(&pts, p), "{class}: {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn dots_fit_inside_discs_and_invalid_spacing_does_not_loop() {
        let shape = Shape::Disc {
            c: [5.0, 3.0],
            r: 20.0,
        };
        let marks = hatch(&shape, "disc", 10.0);
        assert!(!marks.is_empty());
        for mark in marks {
            let HatchMark::Dot(p) = mark else {
                panic!("expected dots")
            };
            assert!((p[0] - 5.0).hypot(p[1] - 3.0) + 1.2 < 20.0);
        }
        for spacing in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(hatch(&shape, "disc", spacing).is_empty());
        }
    }
}
