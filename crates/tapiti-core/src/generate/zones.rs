//! Zoning: the shape class each zone leans toward.

use super::*;

/// The class zone `k` favours: the classes with a positive weight, in turn.
fn zone_class(p: &Params, k: usize) -> Option<ShapeClass> {
    let on: Vec<ShapeClass> = CLASSES
        .iter()
        .copied()
        .filter(|c| p.weights.get(*c) > 0.0)
        .collect();
    (!on.is_empty()).then(|| on[k % on.len()])
}

/// The weight multiplier of class `c` for a room centred at `at`: 6 for the class of its zone, else 1. Always 1 without zoning, so
/// the draws of a floor without zones are the ones it always made.
pub(super) fn factor(p: &Params, at: V, c: ShapeClass) -> f64 {
    if p.zone_count() > 1 && zone_class(p, p.zone_of(at)) == Some(c) {
        6.0
    } else {
        1.0
    }
}
