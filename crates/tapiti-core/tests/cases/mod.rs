//! The named Λ the golden hashes and the native-vs-WebAssembly comparison both run, so the two always cover the same floors.
//! Included by `tests/golden.rs` as a module and by `examples/parity.rs` by path.
#![allow(dead_code)]

use tapiti_core::params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile};

pub fn cases() -> Vec<(String, Params)> {
    let mut out = Vec::new();
    for size in [Size::Small, Size::Medium] {
        out.push((format!("grid-rich-{}", size.name()), rich_of_size(size)));
        let mut growth = rich_of_size(size);
        growth.layout = Layout::Growth;
        out.push((format!("growth-rich-{}", size.name()), growth));
    }
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    out.push(("bsp-rich-medium".into(), bsp));
    for (name, profile) in [
        ("backbone", TopologyProfile::Backbone),
        ("hub", TopologyProfile::Hub),
        ("redundant-exit", TopologyProfile::RedundantExit),
        ("long-loop", TopologyProfile::LongLoop),
        ("spine", TopologyProfile::SpineWithDetours),
        ("ring", TopologyProfile::Ring),
        ("hub-and-wings", TopologyProfile::HubAndWings),
    ] {
        let mut p = params_of_size(Size::Medium);
        p.apply_intent(profile);
        out.push((format!("intent-{name}-medium"), p));
    }
    for (name, layout) in [
        ("grid", Layout::Grid),
        ("growth", Layout::Growth),
        ("bsp", Layout::Bsp),
    ] {
        let mut p = params_of_size(Size::Large);
        p.layout = layout;
        p.set_zones(3, layout != Layout::Grid);
        out.push((format!("zones-{name}-large"), p));
    }
    let mut hills = rich_of_size(Size::Medium);
    hills.environment.height.enabled = true;
    out.push(("height-hills-grid-medium".into(), hills));
    let mut descent = params_of_size(Size::Medium);
    descent.apply_intent(TopologyProfile::SpineWithDetours);
    descent.environment.height.enabled = true;
    descent.environment.height.style = tapiti_core::environment::HeightStyle::Descent;
    out.push(("height-descent-spine-medium".into(), descent));
    out
}
