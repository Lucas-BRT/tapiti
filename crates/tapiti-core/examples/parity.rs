//! One JSON line per case: the Λ and what the WebAssembly module's `stage_json` answers for it (without the clock readings), to
//! compare the native build with the module: `scripts/wasm-parity.sh`.
use serde_json::json;
use tapiti_core::{
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn cases() -> Vec<(String, Params)> {
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
    let mut zones = params_of_size(Size::Large);
    zones.layout = Layout::Growth;
    zones.set_zones(3, true);
    out.push(("zones-growth-large".into(), zones));
    out
}

fn main() {
    for (name, p) in cases() {
        for seed in 1..=3 {
            let s = stage(seed, &p, 8);
            let mut floor = serde_json::to_value(&s.d).unwrap();
            floor.as_object_mut().unwrap().remove("phases");
            println!(
                "{}",
                json!({
                    "name": name,
                    "seed": seed,
                    "params": p,
                    "answer": {
                        "floor": floor,
                        "violations": s.violations,
                        "draws": s.draws,
                        "metrics": metrics_of(&s.d),
                    },
                })
            );
        }
    }
}
