//! Temperature and climate, on top of the height layer (and the water layer, when there is one).
//!
//! The temperature at a node is the mean of the climate plus what the depth adds, plus what the surface adds near an opening to it:
//!
//! $T_i = T_{mean} + g\,\delta_i + o_i\,\sigma\,\varsigma\,(1 - \beta\,u_i)$
//!
//! with $\delta_i$ the depth below the highest ground in hundreds of units, $g$ the gradient (degrees per hundred units of depth),
//! $o_i$ the openness to the surface (1 at an opening, falling with the distance along the corridors to 0 at `reach`), $\sigma$ the
//! swing of the climate, $\varsigma$ the season from $-1$ (winter) to $1$ (summer) and $u_i$ the humidity, whose damping of the swing is
//! $\beta = 1/2$. The entrance is an opening, and so are `openings` rooms more, drawn from a stream of their own and weighted toward
//! high ground. Only sums, products and quotients: the same on every platform.

use super::{fade_from, Environment, Layer, LayerPhase};
use crate::{
    generate::{Floor, Node},
    rng::{Mulberry, Rand},
};
use serde::{Deserialize, Serialize};

/// How much of the surface swing a fully humid node loses.
pub const DAMPING: f64 = 0.5;

/// The climate of the surface: its mean temperature and how far it swings between winter and summer, in degrees Celsius.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Climate {
    Cold,
    #[default]
    Temperate,
    Hot,
}

impl Climate {
    /// `(mean, swing)`.
    pub fn values(self) -> (f64, f64) {
        match self {
            Self::Cold => (-2.0, 14.0),
            Self::Temperate => (12.0, 10.0),
            Self::Hot => (28.0, 8.0),
        }
    }
}

/// The settings of the layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClimateSpec {
    pub enabled: bool,
    pub climate: Climate,
    /// From -1 (winter) to 1 (summer): how much of the swing the surface adds.
    pub season: f64,
    /// Rooms that open to the surface besides the entrance.
    pub openings: usize,
    /// The distance along the corridors at which the surface stops being felt.
    pub reach: f64,
    /// Degrees Celsius gained per hundred units of depth.
    pub gradient: f64,
}

impl Default for ClimateSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            climate: Climate::Temperate,
            season: 0.0,
            openings: 1,
            reach: 500.0,
            gradient: 3.0,
        }
    }
}

impl ClimateSpec {
    /// What is wrong with the settings, if anything.
    pub fn error(&self) -> Option<&'static str> {
        if !(self.season.is_finite() && (-1.0..=1.0).contains(&self.season)) {
            return Some("Climate: the season goes from -1 to 1.");
        }
        if self.openings > 64 {
            return Some("Climate: at most 64 openings.");
        }
        if !(self.reach.is_finite() && self.reach > 0.0) {
            return Some("Climate: the reach must be greater than 0.");
        }
        if !(self.gradient.is_finite() && (0.0..=100.0).contains(&self.gradient)) {
            return Some("Climate: the gradient goes from 0 to 100 degrees per hundred units.");
        }
        None
    }
}

/// The temperature of every node, in the order of `Floor::nodes`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClimateField {
    /// Degrees Celsius.
    pub celsius: Vec<f64>,
    /// 0 (deep inside, farther than `reach` from every opening) to 1 (at an opening).
    pub openness: Vec<f64>,
    /// The nodes that open to the surface: the entrance first, when the floor has one.
    pub openings: Vec<usize>,
}

/// The temperature at one node from its depth (in hundreds of units), openness and humidity.
pub fn temperature(spec: &ClimateSpec, depth: f64, openness: f64, humidity: f64) -> f64 {
    let (mean, swing) = spec.climate.values();
    mean + spec.gradient * depth + openness * swing * spec.season * (1.0 - DAMPING * humidity)
}

/// The depth of every node below the highest ground, in hundreds of units.
pub fn depths(heights: &[f64]) -> Vec<f64> {
    let top = heights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    heights.iter().map(|h| (top - h) / 100.0).collect()
}

/// The layer: it runs after the layout, after the height layer it reads and the water layer it may read.
pub struct ClimateLayer(pub ClimateSpec);

impl ClimateLayer {
    fn field(
        &self,
        floor: &Floor,
        heights: &[f64],
        humidity: Option<&[f64]>,
        rng: &mut Mulberry,
    ) -> ClimateField {
        let spec = self.0;
        let n = floor.nodes.len();
        let room_node = |r: usize| {
            floor
                .nodes
                .iter()
                .position(|node| matches!(node, Node::Room { room } if *room == r))
        };
        let mut openings: Vec<usize> = floor.entrance.and_then(room_node).into_iter().collect();
        let mut rooms: Vec<usize> = (0..n)
            .filter(|&i| {
                matches!(floor.nodes[i], Node::Room { room } if floor.rooms[room].alive)
                    && !openings.contains(&i)
            })
            .collect();
        let lowest = heights.iter().copied().fold(f64::INFINITY, f64::min);
        for _ in 0..spec.openings {
            if rooms.is_empty() {
                break;
            }
            let weights: Vec<f64> = rooms.iter().map(|&i| heights[i] - lowest + 1.0).collect();
            let total: f64 = weights.iter().sum();
            let mut x = rng.next() * total;
            let mut pick = rooms.len() - 1;
            for (k, w) in weights.iter().enumerate() {
                if x < *w {
                    pick = k;
                    break;
                }
                x -= w;
            }
            openings.push(rooms.remove(pick));
        }
        let mut open = vec![false; n];
        for &i in &openings {
            open[i] = true;
        }
        let openness = fade_from(floor, &open, spec.reach);
        let depth = depths(heights);
        let celsius = (0..n)
            .map(|i| temperature(&spec, depth[i], openness[i], humidity.map_or(0.0, |u| u[i])))
            .collect();
        ClimateField {
            celsius,
            openness,
            openings,
        }
    }
}

impl Layer for ClimateLayer {
    fn name(&self) -> &'static str {
        "climate"
    }
    fn tag(&self) -> u32 {
        // "CLIM"
        0x434c_494d
    }
    fn phase(&self) -> LayerPhase {
        LayerPhase::After
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        // The depth needs the ground (`Params::intent_error` says so).
        let Some(height) = out.height.as_ref() else {
            return;
        };
        let heights = height.nodes.clone();
        let humidity = out.water.as_ref().map(|w| w.humidity.clone());
        out.climate = Some(self.field(floor, &heights, humidity.as_deref(), rng));
    }
}

/// The validator's check of the climate (E3): the temperature of every node is the one the formula gives for its depth, openness
/// and humidity, and the openness is a number from 0 to 1.
pub fn check(
    d: &Floor,
    field: &ClimateField,
    heights: &[f64],
    humidity: Option<&[f64]>,
) -> Vec<String> {
    let spec = d.params.environment.climate;
    let n = d.nodes.len();
    let mut out = Vec::new();
    if field.celsius.len() != n || field.openness.len() != n || heights.len() != n {
        out.push("E3 climate: there is not one temperature for each node".to_string());
        return out;
    }
    let depth = depths(heights);
    for (i, ((celsius, o), depth)) in field
        .celsius
        .iter()
        .zip(&field.openness)
        .zip(&depth)
        .enumerate()
    {
        if !(0.0..=1.0).contains(o) {
            out.push(format!("E3 climate: the openness at node {i} is {o}"));
        }
        let want = temperature(&spec, *depth, *o, humidity.map_or(0.0, |u| u[i]));
        if (celsius - want).abs() > 1e-9 * (1.0 + want.abs()) {
            out.push(format!(
                "E3 climate: node {i} is at {celsius:.2} degrees, the formula gives {want:.2}"
            ));
        }
    }
    out
}
