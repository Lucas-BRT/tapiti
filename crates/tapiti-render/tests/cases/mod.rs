//! The named Λ of the 3D fingerprints, shared with `examples/parity_view.rs` (the native-vs-WebAssembly comparison).
#![allow(dead_code)]

use tapiti_core::params::{
    params_of_size, rich_of_size, Layout, LevelsSpec, Params, Size, TopologyProfile,
};

pub fn cases() -> Vec<(String, Params)> {
    let mut out = Vec::new();
    out.push(("grid-rich-small".to_string(), rich_of_size(Size::Small)));
    let mut growth = rich_of_size(Size::Medium);
    growth.layout = Layout::Growth;
    out.push(("growth-rich-medium".into(), growth.clone()));
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    out.push(("bsp-rich-medium".into(), bsp));
    let mut ring = params_of_size(Size::Medium);
    ring.apply_intent(TopologyProfile::Ring);
    out.push(("ring-medium".into(), ring));
    let mut layers = growth;
    layers.environment.height.enabled = true;
    layers.environment.water.enabled = true;
    layers.environment.water.flow = 200.0;
    layers.environment.climate.enabled = true;
    layers.environment.light.enabled = true;
    layers.environment.light.torches = 3;
    out.push(("growth-all-layers-medium".into(), layers.clone()));
    let mut tower = layers;
    tower.levels = LevelsSpec {
        count: 2,
        stairs: 2,
        ..LevelsSpec::default()
    };
    out.push(("two-levels-all-layers-medium".into(), tower));
    out
}
