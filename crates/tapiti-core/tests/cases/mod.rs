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
    let mut ground = rich_of_size(Size::Medium);
    ground.environment.height.enabled = true;
    ground.environment.height.phase = tapiti_core::environment::LayerPhase::Before;
    out.push(("height-before-hills-grid-medium".into(), ground));
    let mut slope = params_of_size(Size::Medium);
    slope.layout = Layout::Growth;
    slope.environment.height.enabled = true;
    slope.environment.height.phase = tapiti_core::environment::LayerPhase::Before;
    slope.environment.height.style = tapiti_core::environment::HeightStyle::Descent;
    out.push(("height-before-descent-growth-medium".into(), slope));
    let mut wet = rich_of_size(Size::Medium);
    wet.layout = Layout::Growth;
    wet.environment.height.enabled = true;
    wet.environment.water.enabled = true;
    out.push(("water-after-growth-medium".into(), wet));
    let mut flood = params_of_size(Size::Medium);
    flood.apply_intent(TopologyProfile::SpineWithDetours);
    flood.environment.height.enabled = true;
    flood.environment.height.phase = tapiti_core::environment::LayerPhase::Before;
    flood.environment.water.enabled = true;
    flood.environment.water.sources = 6;
    flood.environment.water.flow = 400.0;
    out.push(("water-flood-before-height-spine-medium".into(), flood));
    let mut warm = rich_of_size(Size::Medium);
    warm.layout = Layout::Growth;
    warm.environment.height.enabled = true;
    warm.environment.water.enabled = true;
    warm.environment.climate.enabled = true;
    warm.environment.climate.climate = tapiti_core::environment::Climate::Hot;
    warm.environment.climate.season = 1.0;
    out.push(("climate-hot-summer-growth-medium".into(), warm));
    let mut cold = params_of_size(Size::Medium);
    cold.apply_intent(TopologyProfile::Ring);
    cold.environment.height.enabled = true;
    cold.environment.height.phase = tapiti_core::environment::LayerPhase::Before;
    cold.environment.climate.enabled = true;
    cold.environment.climate.climate = tapiti_core::environment::Climate::Cold;
    cold.environment.climate.season = -1.0;
    cold.environment.climate.openings = 3;
    out.push(("climate-cold-winter-before-height-ring-medium".into(), cold));
    let mut torches = rich_of_size(Size::Medium);
    torches.layout = Layout::Growth;
    torches.environment.light.enabled = true;
    torches.environment.light.time = 0.0;
    torches.environment.light.torches = 4;
    torches.environment.light.placement = tapiti_core::environment::Placement::DeadEnds;
    out.push(("light-night-torches-growth-medium".into(), torches));
    let mut sun = params_of_size(Size::Medium);
    sun.apply_intent(TopologyProfile::HubAndWings);
    sun.environment.height.enabled = true;
    sun.environment.light.enabled = true;
    sun.environment.light.time = 9.0;
    sun.environment.light.openings = 2;
    sun.environment.light.torches = 2;
    sun.environment.light.placement = tapiti_core::environment::Placement::HighGround;
    out.push(("light-morning-hub-and-wings-medium".into(), sun));
    use tapiti_core::environment::{EventSpec, Placement};
    let mut scattered = rich_of_size(Size::Medium);
    scattered.layout = Layout::Growth;
    scattered.environment.events.enabled = true;
    scattered.environment.events.list = vec![
        EventSpec {
            name: "trap".into(),
            count: 3,
            placement: Placement::DeadEnds,
            ..EventSpec::default()
        },
        EventSpec {
            name: "guard".into(),
            count: 3,
            placement: Placement::MainPath,
            min_gap: 1,
            ..EventSpec::default()
        },
        EventSpec {
            name: "camp".into(),
            count: 2,
            placement: Placement::FarFromEntrance,
            ..EventSpec::default()
        },
    ];
    out.push(("events-mixed-growth-medium".into(), scattered));
    let mut zoned = params_of_size(Size::Large);
    zoned.layout = Layout::Bsp;
    zoned.set_zones(3, true);
    zoned.environment.height.enabled = true;
    zoned.environment.events.enabled = true;
    zoned.environment.events.exclusive = false;
    zoned.environment.events.list = vec![
        EventSpec {
            name: "spring".into(),
            count: 2,
            placement: Placement::LowGround,
            zone: Some(1),
            ..EventSpec::default()
        },
        EventSpec {
            name: "watch".into(),
            count: 2,
            placement: Placement::HighGround,
            zone: Some(2),
            ..EventSpec::default()
        },
    ];
    out.push(("events-zones-bsp-large".into(), zoned));
    let mut tower = rich_of_size(Size::Medium);
    tower.layout = Layout::Growth;
    tower.levels = tapiti_core::params::LevelsSpec {
        count: 3,
        stairs: 2,
        ..Default::default()
    };
    out.push(("levels-3-growth-medium".into(), tower));
    let mut deep = params_of_size(Size::Medium);
    deep.apply_intent(TopologyProfile::Ring);
    deep.levels.count = 2;
    deep.environment.height.enabled = true;
    deep.environment.water.enabled = true;
    deep.environment.climate.enabled = true;
    deep.environment.light.enabled = true;
    deep.environment.light.torches = 3;
    out.push(("levels-2-ring-all-layers-medium".into(), deep));
    let mut lair = rich_of_size(Size::Medium);
    lair.layout = Layout::Growth;
    lair.environment.height.enabled = true;
    lair.environment.water.enabled = true;
    lair.environment.light.enabled = true;
    lair.environment.light.time = 0.0;
    lair.environment.light.torches = 3;
    lair.environment.events.enabled = true;
    lair.environment.events.list = vec![
        EventSpec {
            name: "damp".into(),
            count: 3,
            conditions: vec![tapiti_core::environment::Condition {
                field: tapiti_core::environment::ConditionField::Humidity,
                min: Some(0.4),
                max: None,
            }],
            ..EventSpec::default()
        },
        EventSpec {
            name: "dark".into(),
            count: 3,
            conditions: vec![tapiti_core::environment::Condition {
                field: tapiti_core::environment::ConditionField::Light,
                min: None,
                max: Some(0.05),
            }],
            ..EventSpec::default()
        },
    ];
    out.push(("events-conditions-growth-medium".into(), lair));
    out
}
