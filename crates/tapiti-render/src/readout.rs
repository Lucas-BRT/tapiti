//! The measures of a floor, read the same way by the desktop app, the website's landing page and the lab: one id, one label, one value
//! format and one explanation for each, so the three say the same words. A bar is given only where the value is a share of a whole that
//! exists: loops of what fits, single-door rooms of the rooms, bridges of the corridors, the way to the exit of the diameter.

use tapiti_core::generate::Floor;
use tapiti_core::metrics::Metrics;
use tapiti_core::params::Params;

/// One measure: a stable id, the label, the value as text, the share of a whole when there is one (0 to 1), and what it means.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
    pub share: Option<f32>,
    pub tip: String,
}

impl Row {
    fn new(
        id: &'static str,
        label: &'static str,
        value: impl Into<String>,
        tip: impl Into<String>,
    ) -> Self {
        Self {
            id,
            label,
            value: value.into(),
            share: None,
            tip: tip.into(),
        }
    }

    fn of(mut self, whole: f64, part: f64) -> Self {
        self.share = (whole > 0.0).then(|| (part / whole).clamp(0.0, 1.0) as f32);
        self
    }
}

/// The five rules the validator checks, each as its code, a name and what it checks, in few words.
pub const RULES: [(&str, &str, &str); 5] = [
    ("V1", "connectivity", "every room reached"),
    ("V2", "clearances", "rooms and corridors apart"),
    ("V3", "doors", "fit and separated"),
    ("V4", "corridors", "angles, bends, lengths"),
    ("V5", "entrance, exit", "present, far enough"),
];

/// The measures of a floor, in the order they are read.
pub fn rows(m: &Metrics) -> Vec<Row> {
    let rooms = (m.rooms + m.leaf_chambers) as f64;
    let dash = || "—".to_string();
    vec![
        Row::new(
            "rooms",
            "rooms",
            format!("{}+{}", m.rooms, m.leaf_chambers),
            format!(
                "{} main rooms and {} leaf chambers, linked by a single corridor.",
                m.rooms, m.leaf_chambers
            ),
        ),
        Row::new(
            "corridors",
            "corridors",
            m.edges.to_string(),
            "Pieces of corridor in the graph; each arm of a junction counts as one.",
        ),
        Row::new(
            "loops",
            "loops β",
            format!("{} / {}", m.beta, m.beta_max),
            format!(
                "Extra ways round, instead of back: {} now, {} aimed for by the density, room for {}. A tree has none. The bar is how much of that room is used.",
                m.beta, m.beta_target, m.beta_max
            ),
        )
        .of(f64::from(m.beta_max), f64::from(m.beta)),
        Row::new(
            "single_door",
            "single-door rooms",
            m.leaves.to_string(),
            "Rooms with a single door: the leaf chambers, the entrance and the exit. The bar is their share of all the rooms.",
        )
        .of(rooms, m.leaves as f64),
        Row::new(
            "bridges",
            "bridges",
            format!("{} · {:.0}%", m.bridges, m.bridge_share * 100.0),
            "Corridors whose removal would cut part of the floor off. The bar is their share of the corridors.",
        )
        .of(1.0, m.bridge_share),
        {
            let mut row = Row::new(
                "steps_to_exit",
                "steps to exit",
                m.hops_to_exit.map_or_else(dash, |h| format!("{h:.0}")),
                "Rooms on the shortest way from the entrance to the exit. The bar is that way as a share of the diameter.",
            );
            // A floor with no entrance and exit has no way to show, so no bar either.
            if let Some(h) = m.hops_to_exit {
                row = row.of(m.diameter, h);
            }
            row
        },
        Row::new(
            "routes",
            "routes → exit",
            m.routes_to_exit.map_or_else(dash, |n| n.to_string()),
            "Ways from the entrance to the exit that share no corridor.",
        ),
        Row::new(
            "diameter",
            "diameter",
            format!("{:.0}", m.diameter),
            "The most rooms between two rooms, by the shortest way.",
        ),
        Row::new(
            "lambda2",
            "λ₂",
            format!("{:.3}", m.lambda2),
            "Connectivity: larger means less depending on a few corridors.",
        ),
        Row::new(
            "doors_per_room",
            "doors per room",
            format!("{:.2}", m.mean_degree),
            "Corridors per room, on average.",
        ),
    ]
}

/// FNV-1a of a byte string, as eight hex digits.
fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(16_777_619);
    }
    format!("{h:08x}")
}

/// A hash of a seed and the Λ of a floor, so two windows (a desktop app, a browser) that show the same hash show the same floor.
pub fn floor_hash(seed: u32, params: &Params) -> String {
    let text = serde_json::to_string(params).unwrap_or_default();
    fnv1a(format!("{seed}{text}").as_bytes())
}

/// The Λ the floor was drawn with, hashed with its seed.
pub fn hash_of(seed: u32, floor: &Floor) -> String {
    floor_hash(seed, &floor.params)
}

/// The Λ of a size (1 small to 4 huge) with every group present, as JSON: the plain `params_of_size` leaves out the zones, the levels
/// and the environment while they are off, but a form needs their fields, at their defaults, to edit them. The lab and the parameter
/// reference both read the defaults from here.
pub fn full_params(size: usize) -> serde_json::Value {
    use tapiti_core::params::{params_of_size, LevelsSpec, Size, ZoneSpec};
    let mut value = serde_json::to_value(params_of_size(Size::of_index(size))).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        object
            .entry("zones")
            .or_insert_with(|| serde_json::to_value(ZoneSpec::default()).unwrap_or_default());
        object
            .entry("levels")
            .or_insert_with(|| serde_json::to_value(LevelsSpec::default()).unwrap_or_default());
        object.entry("environment").or_insert_with(|| {
            serde_json::to_value(tapiti_core::environment::EnvSpec::default()).unwrap_or_default()
        });
    }
    value
}
