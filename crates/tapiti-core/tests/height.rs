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
