//! Floors drawn by the generator itself and inlined in pages as SVG.

use std::sync::OnceLock;
use std::time::Instant;
use tapiti_core::{
    generate::Floor,
    params::{Size, rich_of_size},
    stage::stage,
};
use tapiti_render::{Style, floor_svg};
use topcoat::{
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

/// Markup produced by this repository's own code (generator output, repository docs), never from request data, so it is rendered unescaped.
pub struct TrustedHtml(pub String);

impl NodeViewParts for TrustedHtml {
    fn into_view_parts(self, _cx: &Cx, parts: &mut PartsWriter<'_>) {
        parts.push_string_unescaped(self.0);
    }
}

/// The floor the landing page walks along: the medium size with every option on, first draw valid, a main path of ten rooms through all five shapes, one
/// junction and a leaf chamber. The scene of phase 5 builds the same floor in the browser from the same seed.
pub const WALK_SEED: u32 = 279;
pub const WALK_SIZE: Size = Size::Medium;
/// Its position among the sizes (1 to 4), as the wasm module takes it.
pub const WALK_SIZE_INDEX: usize = 2;
/// Sections of the landing page, one room each.
pub const STATIONS: usize = 7;

fn svg(floor: &Floor, label: String) -> TrustedHtml {
    TrustedHtml(floor_svg(
        floor,
        &Style {
            label: Some(label),
            ..Style::default()
        },
    ))
}

/// The floor of the walk (a validated stage, as the browser's wasm module returns it for the same seed).
pub fn walk() -> &'static Floor {
    static FLOOR: OnceLock<Floor> = OnceLock::new();
    FLOOR.get_or_init(|| stage(WALK_SEED, &rich_of_size(WALK_SIZE), 8).d)
}

pub fn walk_svg(label: String) -> TrustedHtml {
    svg(walk(), label)
}

/// Numbers measured on this machine when the server starts, so the landing page never quotes a figure the code did not produce.
#[derive(Clone, Copy, Debug)]
pub struct Speed {
    /// Median time to dig a validated medium floor, in milliseconds.
    pub median_ms: u32,
    pub valid: usize,
    pub total: usize,
}

pub fn speed() -> Speed {
    static SPEED: OnceLock<Speed> = OnceLock::new();
    *SPEED.get_or_init(|| {
        let params = rich_of_size(Size::Medium);
        let total = 60;
        let mut times = Vec::with_capacity(total);
        let mut valid = 0;
        for seed in 1..=total as u32 {
            let started = Instant::now();
            let staged = stage(seed, &params, 8);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            valid += usize::from(staged.violations.is_empty());
        }
        times.sort_by(f64::total_cmp);
        Speed {
            median_ms: times[times.len() / 2].round().max(1.0) as u32,
            valid,
            total,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{generate::Node, validate::validate};
    use tapiti_render::view_box;

    /// A room of the walk where the page stops, with where it sits on the SVG (percentages of the view box).
    #[derive(Clone, Copy, Debug)]
    pub struct Station {
        pub room: usize,
        pub left_pct: f64,
        pub top_pct: f64,
    }

    /// The rooms along the main path, from the entrance to the exit.
    pub fn path_rooms(floor: &Floor) -> Vec<usize> {
        floor
            .main_path
            .iter()
            .filter_map(|&node| match floor.nodes.get(node) {
                Some(Node::Room { room }) => Some(*room),
                _ => None,
            })
            .collect()
    }

    /// Seven rooms spread over the main path: room `round(i * (K - 1) / 6)` of its `K` rooms.
    pub fn stations() -> [Station; STATIONS] {
        let floor = walk();
        let path = path_rooms(floor);
        let [vx, vy, vw, vh] = view_box(floor, &Style::default());
        let last = (path.len() - 1) as f64;
        std::array::from_fn(|i| {
            let room = path[(i as f64 * last / (STATIONS - 1) as f64).round() as usize];
            let [x, y] = floor.rooms[room].center;
            Station {
                room,
                left_pct: (x - vx) / vw * 100.0,
                top_pct: (y - vy) / vh * 100.0,
            }
        })
    }

    #[test]
    fn the_walk_floor_has_seven_stations() {
        let floor = walk();
        assert!(validate(floor).is_empty(), "{:?}", validate(floor));
        let path = path_rooms(floor);
        assert!(path.len() >= STATIONS, "{}", path.len());
        let stations = stations();
        assert_eq!(stations[0].room, path[0]);
        assert_eq!(stations[STATIONS - 1].room, *path.last().unwrap());
        assert!(stations.windows(2).all(|pair| pair[0].room != pair[1].room));
        // Entrance first, exit last, and every shape on the way.
        let shapes: std::collections::HashSet<&str> = stations
            .iter()
            .map(|s| floor.rooms[s.room].cls.name())
            .collect();
        assert_eq!(shapes.len(), 5, "{shapes:?}");
    }

    #[test]
    fn stations_are_inside_the_viewbox() {
        for station in stations() {
            assert!((0.0..=100.0).contains(&station.left_pct), "{station:?}");
            assert!((0.0..=100.0).contains(&station.top_pct), "{station:?}");
        }
    }
}
