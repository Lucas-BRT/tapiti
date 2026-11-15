//! The view model of the environment layers: for each channel, a number per room, the range it spans, a colour ramp and the lines the
//! inspector shows for a room. Every renderer (the desktop map, the SVG, a 3D view) reads these, so a channel looks the same everywhere.

use tapiti_core::generate::{Floor, Node};

/// What a renderer can tint rooms by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// The height of the floor.
    Height,
    /// The depth of the water over the floor.
    Water,
    /// The humidity, 0 to 1.
    Humidity,
    /// The temperature, in degrees Celsius.
    Temperature,
    /// The light, 0 to 1.
    Light,
}

impl Channel {
    pub const ALL: [Channel; 5] = [
        Self::Height,
        Self::Water,
        Self::Humidity,
        Self::Temperature,
        Self::Light,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Height => "height",
            Self::Water => "water",
            Self::Humidity => "humidity",
            Self::Temperature => "temperature",
            Self::Light => "light",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Height => "Height",
            Self::Water => "Water depth",
            Self::Humidity => "Humidity",
            Self::Temperature => "Temperature",
            Self::Light => "Light",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Height | Self::Water => "",
            Self::Humidity | Self::Light => "%",
            Self::Temperature => "°C",
        }
    }

    /// Whether the floor carries the layer this channel reads.
    pub fn available(self, floor: &Floor) -> bool {
        let Some(env) = floor.environment.as_ref() else {
            return false;
        };
        match self {
            Self::Height => env.height.is_some(),
            Self::Water | Self::Humidity => env.water.is_some() && env.height.is_some(),
            Self::Temperature => env.climate.is_some(),
            Self::Light => env.light.is_some(),
        }
    }
}

/// The value of the channel at every node, in the order of `Floor::nodes` (`None` where the layer is off).
fn node_values(floor: &Floor, c: Channel) -> Option<Vec<f64>> {
    let env = floor.environment.as_ref()?;
    match c {
        Channel::Height => env.height.as_ref().map(|h| h.nodes.clone()),
        Channel::Water => {
            let (w, h) = (env.water.as_ref()?, env.height.as_ref()?);
            Some(
                w.level
                    .iter()
                    .zip(&h.nodes)
                    .map(|(l, h)| (l - h).max(0.0))
                    .collect(),
            )
        }
        Channel::Humidity => env.water.as_ref().map(|w| w.humidity.clone()),
        Channel::Temperature => env.climate.as_ref().map(|t| t.celsius.clone()),
        Channel::Light => env.light.as_ref().map(|l| l.intensity.clone()),
    }
}

/// The value of the channel in every room, by room id; `None` for a dead room or when the layer is off.
pub fn room_values(floor: &Floor, c: Channel) -> Vec<Option<f64>> {
    let mut out = vec![None; floor.rooms.len()];
    if let Some(values) = node_values(floor, c) {
        for (node, v) in floor.nodes.iter().zip(values) {
            if let Node::Room { room } = node {
                if floor.rooms[*room].alive {
                    out[*room] = Some(v);
                }
            }
        }
    }
    out
}

/// The range the colours span: the rooms' least and greatest value, except humidity and light, which always span 0 to 1, and water,
/// which spans 0 to the deepest. `None` when no room has a value.
pub fn scale(floor: &Floor, c: Channel) -> Option<(f64, f64)> {
    match c {
        Channel::Humidity | Channel::Light => {
            return room_values(floor, c)
                .iter()
                .any(Option::is_some)
                .then_some((0.0, 1.0));
        }
        _ => {}
    }
    let values: Vec<f64> = room_values(floor, c).into_iter().flatten().collect();
    if values.is_empty() {
        return None;
    }
    let (lo, hi) = (
        values.iter().copied().fold(f64::INFINITY, f64::min),
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    );
    Some(if c == Channel::Water {
        (0.0, hi)
    } else {
        (lo, hi)
    })
}

/// The colour of a value at `t` from 0 (the low end of the scale) to 1 (the high end).
pub fn color(c: Channel, t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let ramp =
        |a: [f64; 3], b: [f64; 3]| [0, 1, 2].map(|i| (a[i] + (b[i] - a[i]) * t).round() as u8);
    match c {
        // Blue low ground, amber high.
        Channel::Height => ramp([63.0, 111.0, 168.0], [217.0, 138.0, 58.0]),
        // Pale to deep blue.
        Channel::Water => ramp([160.0, 205.0, 235.0], [31.0, 127.0, 201.0]),
        // Dry sand to a damp teal.
        Channel::Humidity => ramp([226.0, 214.0, 180.0], [46.0, 140.0, 140.0]),
        // Cold blue to hot red.
        Channel::Temperature => ramp([47.0, 109.0, 196.0], [208.0, 58.0, 44.0]),
        // Dark to warm light.
        Channel::Light => ramp([20.0, 20.0, 28.0], [255.0, 236.0, 170.0]),
    }
}

/// `t` of a value on the scale of its channel, 0.5 when the scale is a single value.
pub fn position(scale: (f64, f64), v: f64) -> f64 {
    let span = scale.1 - scale.0;
    if span.abs() < 1e-12 {
        0.5
    } else {
        ((v - scale.0) / span).clamp(0.0, 1.0)
    }
}

/// The value as the inspector prints it.
pub fn format_value(c: Channel, v: f64) -> String {
    match c {
        Channel::Humidity | Channel::Light => format!("{:.0}%", 100.0 * v),
        Channel::Temperature => format!("{v:.1}°C"),
        Channel::Height | Channel::Water => format!("{v:.0}"),
    }
}

/// The lines the inspector shows for a room besides its shape: the level, the value of every channel the floor has, and the events
/// that happen in it.
pub fn inspect(floor: &Floor, room: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if floor.stack.is_some() {
        out.push((
            "Level".to_string(),
            (floor.rooms[room].level + 1).to_string(),
        ));
    }
    for c in Channel::ALL {
        if let Some(v) = room_values(floor, c).get(room).copied().flatten() {
            if c == Channel::Water && v <= 1e-9 {
                continue;
            }
            out.push((c.label().to_string(), format_value(c, v)));
        }
    }
    if let (Some(events), Some(node)) = (
        floor.environment.as_ref().and_then(|e| e.events.as_ref()),
        floor
            .nodes
            .iter()
            .position(|n| matches!(n, Node::Room { room: r } if *r == room)),
    ) {
        let here: Vec<&str> = events
            .kinds
            .iter()
            .filter(|k| k.nodes.contains(&node))
            .map(|k| k.name.as_str())
            .collect();
        if !here.is_empty() {
            out.push(("Events".to_string(), here.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{
        generate::generate,
        params::{rich_of_size, Layout, Size},
    };

    fn everything() -> Floor {
        let mut p = rich_of_size(Size::Medium);
        p.layout = Layout::Growth;
        p.environment.height.enabled = true;
        p.environment.water.enabled = true;
        p.environment.water.flow = 300.0;
        p.environment.climate.enabled = true;
        p.environment.light.enabled = true;
        p.environment.light.torches = 2;
        generate(3, &p)
    }

    #[test]
    fn a_floor_with_no_layer_has_no_channel() {
        let f = generate(3, &rich_of_size(Size::Medium));
        for c in Channel::ALL {
            assert!(!c.available(&f));
            assert!(scale(&f, c).is_none());
            assert!(room_values(&f, c).iter().all(Option::is_none));
        }
        assert!(inspect(&f, 0).is_empty());
    }

    #[test]
    fn every_channel_of_a_full_floor_has_values_inside_its_scale_and_a_colour() {
        let f = everything();
        for c in Channel::ALL {
            assert!(c.available(&f), "{c:?}");
            let (lo, hi) = scale(&f, c).unwrap();
            assert!(lo <= hi);
            for v in room_values(&f, c).into_iter().flatten() {
                assert!(v >= lo - 1e-9 && v <= hi + 1e-9, "{c:?}");
                let t = position((lo, hi), v);
                assert!((0.0..=1.0).contains(&t));
                let _ = color(c, t);
            }
            assert_ne!(color(c, 0.0), color(c, 1.0), "{c:?}: a ramp of one colour");
        }
        let lines = inspect(&f, f.entrance.unwrap());
        assert!(
            lines.iter().any(|(k, _)| k == "Height") && lines.iter().any(|(k, _)| k == "Light")
        );
    }
}
