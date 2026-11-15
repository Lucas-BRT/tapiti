//! Environment layers: fields laid over a floor (height, water, climate, light, events), each its own pass.
//!
//! Rules every layer keeps, so that a layer that is off costs nothing and one that is on never moves a room:
//! - its settings are left out of the JSON while it is off (`EnvSpec::is_off`), and so is `Floor::environment`;
//! - it draws from a stream of its own, seeded from the seed and a tag of the layer ([`layer_rng`]), never from the generator's;
//! - it uses integer or polynomial arithmetic, no `sin`, `cos`, `ln` or `exp`, so the same seed gives the same field on every platform;
//! - a layer runs **after** the layout and only reads the finished floor, or **before** it and hands the generator limits to respect
//!   ([`LayerPhase`]). A layer that runs before changes the floor when it is on, and only then.

use crate::{generate::Floor, params::Params, rng::Mulberry};
use serde::{Deserialize, Serialize};

/// The rectangle of the world, in the units of the floor, handed to the layers that run before the rooms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// When a layer runs relative to the layout of the rooms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayerPhase {
    /// Reads the finished floor and annotates it; no room moves.
    #[default]
    After,
    /// Runs before the rooms are placed and limits where they and their corridors may go.
    Before,
}

/// The settings every layer has: whether it runs, and when.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayerSpec {
    pub enabled: bool,
    pub phase: LayerPhase,
}

/// The layers' settings in Λ. Left out of the JSON while every layer is off, so older files keep their shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvSpec {}

impl EnvSpec {
    pub fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

/// What the layers computed, carried by the floor. Left out of the JSON while no layer ran.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {}

/// The stream of a layer: a pure integer mix of the seed and the layer's tag (SplitMix32), so it is the same on every platform and
/// no layer shares draws with the generator or with another layer.
pub fn layer_rng(seed: u32, tag: u32) -> Mulberry {
    let mut z = seed ^ tag.wrapping_mul(0x9e3779b9);
    z = (z ^ (z >> 16)).wrapping_mul(0x85ebca6b);
    z = (z ^ (z >> 13)).wrapping_mul(0xc2b2ae35);
    Mulberry(z ^ (z >> 16))
}

/// One layer. Each layer implements the phase it runs in and leaves the other as it is.
pub trait Layer {
    fn name(&self) -> &'static str;
    /// A number that never changes, so the layer's stream stays its own.
    fn tag(&self) -> u32;
    fn phase(&self) -> LayerPhase;
    /// Before the rooms: reads the world and fills `out`, which the generator then reads.
    fn before(&self, _world: World, _rng: &mut Mulberry, _out: &mut Environment) {}
    /// After the layout: reads the finished floor and fills `out`.
    fn after(&self, _floor: &Floor, _rng: &mut Mulberry, _out: &mut Environment) {}
}

/// The layers `p` switches on, in the order they run (each reads only the layers before it).
pub fn layers_of(_p: &Params) -> Vec<Box<dyn Layer>> {
    Vec::new()
}

/// Runs the layers that go before the rooms. `None` when none did.
pub fn run_before(layers: &[Box<dyn Layer>], seed: u32, world: World) -> Option<Environment> {
    let mut out = Environment::default();
    let mut ran = false;
    for layer in layers.iter().filter(|l| l.phase() == LayerPhase::Before) {
        layer.before(world, &mut layer_rng(seed, layer.tag()), &mut out);
        ran = true;
    }
    ran.then_some(out)
}

/// Runs the layers that go after the layout, on top of what the earlier ones left. `None` when no layer ran at all.
pub fn run_after(
    layers: &[Box<dyn Layer>],
    seed: u32,
    floor: &Floor,
    before: Option<Environment>,
) -> Option<Environment> {
    let mut out = before.unwrap_or_default();
    let mut ran = !layers.is_empty();
    for layer in layers.iter().filter(|l| l.phase() == LayerPhase::After) {
        layer.after(floor, &mut layer_rng(seed, layer.tag()), &mut out);
        ran = true;
    }
    ran.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        generate::generate,
        params::{rich_of_size, Size},
        rng::Rand,
    };

    #[test]
    fn a_layers_stream_is_its_own_and_repeats() {
        let first: Vec<f64> = {
            let mut r = layer_rng(7, 1);
            (0..4).map(|_| r.next()).collect()
        };
        let again: Vec<f64> = {
            let mut r = layer_rng(7, 1);
            (0..4).map(|_| r.next()).collect()
        };
        let other_tag = layer_rng(7, 2).next();
        let other_seed = layer_rng(8, 1).next();
        let generator = Mulberry(7).next();
        assert_eq!(first, again);
        assert!(first[0] != other_tag && first[0] != other_seed && first[0] != generator);
    }

    #[test]
    fn no_layer_means_no_environment_in_the_floor_or_the_lambda() {
        let p = rich_of_size(Size::Small);
        assert!(p.environment.is_off());
        let floor = serde_json::to_value(generate(1, &p)).unwrap();
        assert!(floor.get("environment").is_none());
        assert!(serde_json::to_value(&p)
            .unwrap()
            .get("environment")
            .is_none());
    }

    /// A layer that only counts: it must see the finished floor, draw from its own stream and leave every room where it was.
    struct Probe(LayerPhase);
    impl Layer for Probe {
        fn name(&self) -> &'static str {
            "probe"
        }
        fn tag(&self) -> u32 {
            0xfeed
        }
        fn phase(&self) -> LayerPhase {
            self.0
        }
        fn after(&self, floor: &Floor, rng: &mut Mulberry, _out: &mut Environment) {
            assert!(!floor.rooms.is_empty());
            rng.next();
        }
    }

    #[test]
    fn layers_run_in_their_phase_and_only_after_runs_on_the_floor() {
        let p = rich_of_size(Size::Small);
        let floor = generate(3, &p);
        let before_only: Vec<Box<dyn Layer>> = vec![Box::new(Probe(LayerPhase::Before))];
        let after: Vec<Box<dyn Layer>> = vec![Box::new(Probe(LayerPhase::After))];
        let world = World {
            x0: 0.0,
            y0: 0.0,
            x1: 1.0,
            y1: 1.0,
        };
        assert!(run_before(&after, 3, world).is_none());
        assert!(run_before(&before_only, 3, world).is_some());
        assert!(run_after(&after, 3, &floor, None).is_some());
        assert!(run_after(&[], 3, &floor, None).is_none());
    }
}
