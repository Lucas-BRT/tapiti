use tapiti_core::{
    environment::{height::edge_slopes, HeightStyle},
    generate::generate,
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn with_height(mut p: Params, style: HeightStyle) -> Params {
    p.environment.height.enabled = true;
    p.environment.height.style = style;
    p
}

/// The floor as JSON without the clock, the Λ and the environment: what a layer must not change.
fn bare(seed: u32, p: &Params) -> serde_json::Value {
    let mut v = serde_json::to_value(generate(seed, p)).unwrap();
    let o = v.as_object_mut().unwrap();
    for key in ["phases", "params", "environment"] {
        o.remove(key);
    }
    v
}

fn lambdas() -> Vec<(&'static str, Params)> {
    let mut growth = rich_of_size(Size::Medium);
    growth.layout = Layout::Growth;
    let mut spine = params_of_size(Size::Medium);
    spine.apply_intent(TopologyProfile::SpineWithDetours);
    let mut ring = params_of_size(Size::Medium);
    ring.apply_intent(TopologyProfile::Ring);
    vec![
        ("grid", rich_of_size(Size::Medium)),
        ("growth", growth),
        ("spine", spine),
        ("ring", ring),
    ]
}

#[test]
fn height_after_the_layout_moves_no_room() {
    for (name, plain) in lambdas() {
        for style in [HeightStyle::Hills, HeightStyle::Descent] {
            let tall = with_height(plain.clone(), style);
            for seed in 1..=8 {
                assert_eq!(
                    bare(seed, &plain),
                    bare(seed, &tall),
                    "{name} {style:?} seed {seed}"
                );
            }
        }
    }
}

#[test]
fn every_corridor_keeps_within_the_slope_and_the_validator_agrees() {
    for (name, plain) in lambdas() {
        for style in [HeightStyle::Hills, HeightStyle::Descent] {
            let p = with_height(plain.clone(), style);
            for seed in 1..=8 {
                let floor = generate(seed, &p);
                let field = floor
                    .environment
                    .as_ref()
                    .and_then(|e| e.height.as_ref())
                    .expect("a height field");
                assert_eq!(field.nodes.len(), floor.nodes.len());
                for slope in edge_slopes(&floor, field) {
                    assert!(
                        slope <= p.environment.height.max_slope * (1.0 + 1e-9) + 1e-12,
                        "{name} {style:?} seed {seed}: {slope}"
                    );
                }
                let s = stage(seed, &p, 1);
                assert!(
                    !s.violations.iter().any(|v| v.starts_with("E1")),
                    "{name} {style:?} seed {seed}: {:?}",
                    s.violations
                );
                let m = metrics_of(&s.d).height.expect("height measures");
                assert!(m.max_slope <= p.environment.height.max_slope * (1.0 + 1e-9) + 1e-12);
                assert!(
                    m.range >= 0.0 && (m.range - (field.max - field.min)).abs() < 1e-9
                        || s.draws > 1
                );
            }
        }
    }
}

#[test]
fn hills_have_relief_and_descent_goes_down() {
    let growth = lambdas().remove(1).1;
    let hills = with_height(growth.clone(), HeightStyle::Hills);
    let range = |p: &Params| {
        (1..=8)
            .map(|seed| metrics_of(&generate(seed, p)).height.unwrap().range)
            .fold(0.0, f64::max)
    };
    assert!(range(&hills) > 10.0, "hills are flat");
    let descent = with_height(growth, HeightStyle::Descent);
    for seed in 1..=8 {
        let floor = generate(seed, &descent);
        let field = floor.environment.as_ref().unwrap().height.as_ref().unwrap();
        let entrance = floor.nodes.iter().position(|n| matches!(n, tapiti_core::generate::Node::Room { room } if Some(*room) == floor.entrance)).unwrap();
        assert!(
            field
                .nodes
                .iter()
                .all(|h| *h <= field.nodes[entrance] + 1e-9),
            "seed {seed}: the entrance is the highest point"
        );
    }
}

#[test]
fn the_same_seed_gives_the_same_heights_and_the_lambda_round_trips() {
    let p = with_height(rich_of_size(Size::Medium), HeightStyle::Hills);
    let a = serde_json::to_value(generate(5, &p).environment).unwrap();
    let b = serde_json::to_value(generate(5, &p).environment).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["environment"]["height"]["enabled"], true);
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, p.environment);
    let off = serde_json::to_value(rich_of_size(Size::Medium)).unwrap();
    assert!(off.get("environment").is_none());
}

#[test]
fn bad_settings_are_named() {
    let mut p = with_height(params_of_size(Size::Small), HeightStyle::Hills);
    assert!(p.intent_error().is_none());
    p.environment.height.max_slope = 0.0;
    assert!(p.intent_error().is_some());
    p.environment.height.max_slope = 0.25;
    p.environment.height.relief = -1.0;
    assert!(p.intent_error().is_some());
}

use tapiti_core::environment::{height::terrain_of, LayerPhase, World};

fn before(mut p: Params, style: HeightStyle) -> Params {
    p = with_height(p, style);
    p.environment.height.phase = LayerPhase::Before;
    p
}

fn all_layouts() -> Vec<(&'static str, Params)> {
    let mut out = lambdas();
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    let mut hub = params_of_size(Size::Medium);
    hub.apply_intent(TopologyProfile::HubAndWings);
    let mut zones = params_of_size(Size::Large);
    zones.layout = Layout::Growth;
    zones.set_zones(3, true);
    out.extend([("bsp", bsp), ("hub-and-wings", hub), ("zones", zones)]);
    out
}

#[test]
fn the_ground_is_a_pure_function_of_the_position() {
    let p = before(params_of_size(Size::Medium), HeightStyle::Hills);
    let world = World {
        x0: 0.0,
        y0: 0.0,
        x1: 1620.0,
        y1: 1150.0,
    };
    let a = terrain_of(&p, 7, world).unwrap();
    let b = terrain_of(&p, 7, world).unwrap();
    let other = terrain_of(&p, 8, world).unwrap();
    let spots = [
        [10.0, 10.0],
        [800.5, 300.25],
        [1600.0, 1100.0],
        [-40.0, 90.0],
    ];
    for s in spots {
        assert_eq!(a.at(s), b.at(s));
        assert!((0.0..=p.environment.height.relief).contains(&a.at(s)));
    }
    assert!(spots.iter().any(|s| a.at(*s) != other.at(*s)));
    assert!(terrain_of(&params_of_size(Size::Medium), 7, world).is_none());
    let down = terrain_of(
        &before(params_of_size(Size::Medium), HeightStyle::Descent),
        7,
        world,
    )
    .unwrap();
    assert!(
        down.at([0.0, 500.0]) > down.at([800.0, 500.0])
            && down.at([800.0, 500.0]) > down.at([1620.0, 500.0])
    );
}

#[test]
fn height_before_the_layout_keeps_the_rule_and_says_how_far_it_lowered_a_room() {
    for (name, plain) in all_layouts() {
        for style in [HeightStyle::Hills, HeightStyle::Descent] {
            let p = before(plain.clone(), style);
            let mut lowest = 0.0f64;
            for seed in 1..=8 {
                let s = stage(seed, &p, 8);
                let field =
                    s.d.environment
                        .as_ref()
                        .and_then(|e| e.height.as_ref())
                        .expect("a height field");
                assert!(
                    !s.violations.iter().any(|v| v.starts_with("E1")),
                    "{name} {style:?} seed {seed}: {:?}",
                    s.violations
                );
                lowest = lowest.max(
                    field
                        .lowered
                        .expect("lowered is measured before the layout"),
                );
                assert_eq!(
                    serde_json::to_value(generate(seed, &p).environment).unwrap(),
                    serde_json::to_value(generate(seed, &p).environment).unwrap()
                );
            }
            println!("before {name} {style:?}: rooms lowered by at most {lowest:.1}");
        }
    }
}

#[test]
fn how_often_a_floor_stays_valid_with_the_ground_in_the_way() {
    for (name, plain) in all_layouts() {
        let p = before(plain.clone(), HeightStyle::Hills);
        let valid = (1..=20)
            .filter(|seed| stage(*seed, &p, 8).violations.is_empty())
            .count();
        let off = (1..=20)
            .filter(|seed| stage(*seed, &plain, 8).violations.is_empty())
            .count();
        println!("height before, {name}: valid {valid}/20 (off: {off}/20)");
    }
}
