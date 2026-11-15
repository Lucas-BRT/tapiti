use tapiti_core::{
    environment::{HeightStyle, LayerPhase},
    generate::generate,
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn with_water(mut p: Params, phase: LayerPhase, style: HeightStyle) -> Params {
    p.environment.height.enabled = true;
    p.environment.height.phase = phase;
    p.environment.height.style = style;
    p.environment.water.enabled = true;
    p
}

fn harsh(mut p: Params) -> Params {
    p.environment.water.sources = 8;
    p.environment.water.flow = 400.0;
    p
}

fn lambdas() -> Vec<(&'static str, Params)> {
    let mut growth = rich_of_size(Size::Medium);
    growth.layout = Layout::Growth;
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    let mut spine = params_of_size(Size::Large);
    spine.apply_intent(TopologyProfile::SpineWithDetours);
    let mut ring = params_of_size(Size::Medium);
    ring.apply_intent(TopologyProfile::Ring);
    vec![
        ("grid", rich_of_size(Size::Medium)),
        ("growth", growth),
        ("bsp", bsp),
        ("spine", spine),
        ("ring", ring),
    ]
}

fn bare(seed: u32, p: &Params) -> serde_json::Value {
    let mut v = serde_json::to_value(generate(seed, p)).unwrap();
    let o = v.as_object_mut().unwrap();
    for key in ["phases", "params", "environment"] {
        o.remove(key);
    }
    v
}

#[test]
fn water_moves_no_room_and_the_height_stays_what_it_was() {
    for (name, plain) in lambdas() {
        let mut tall = plain.clone();
        tall.environment.height.enabled = true;
        let wet = with_water(plain.clone(), LayerPhase::After, HeightStyle::Hills);
        for seed in 1..=6 {
            assert_eq!(bare(seed, &tall), bare(seed, &wet), "{name} seed {seed}");
            let a = generate(seed, &tall).environment.unwrap().height;
            let b = generate(seed, &wet).environment.unwrap().height;
            assert_eq!(a, b, "{name} seed {seed}: the water changed the heights");
        }
    }
}

#[test]
fn the_water_poured_is_the_water_held_and_it_never_runs_uphill() {
    let mut any_wet = false;
    for (name, plain) in lambdas() {
        for phase in [LayerPhase::After, LayerPhase::Before] {
            for style in [HeightStyle::Hills, HeightStyle::Descent] {
                for p in [
                    with_water(plain.clone(), phase, style),
                    harsh(with_water(plain.clone(), phase, style)),
                ] {
                    for seed in 1..=6 {
                        let s = stage(seed, &p, 1);
                        let tag = format!(
                            "{name} {phase:?} {style:?} flow {} seed {seed}",
                            p.environment.water.flow
                        );
                        assert!(
                            !s.violations.iter().any(|v| v.starts_with("E")),
                            "{tag}: {:?}",
                            s.violations
                        );
                        let env = s.d.environment.as_ref().unwrap();
                        let (water, height) =
                            (env.water.as_ref().unwrap(), env.height.as_ref().unwrap());
                        for route in &water.routes {
                            for pair in route.windows(2) {
                                assert!(
                                    height.nodes[pair[1]] < height.nodes[pair[0]],
                                    "{tag}: water ran uphill"
                                );
                            }
                        }
                        for (i, l) in water.level.iter().enumerate() {
                            assert!(*l >= height.nodes[i] - 1e-9, "{tag}: level under the floor");
                            assert!(
                                (0.0..=1.0).contains(&water.humidity[i]),
                                "{tag}: humidity out of range"
                            );
                            if *l - height.nodes[i] > 1e-6 {
                                assert_eq!(
                                    water.humidity[i], 1.0,
                                    "{tag}: a wet node is not at humidity 1"
                                );
                            }
                        }
                        let m = metrics_of(&s.d).water.expect("water measures");
                        any_wet |= m.wet_rooms > 0;
                        assert_eq!(
                            m.sources,
                            p.environment
                                .water
                                .sources
                                .min(s.d.rooms.iter().filter(|r| r.alive).count())
                        );
                    }
                }
            }
        }
    }
    assert!(any_wet, "no room was ever under water");
}

#[test]
fn the_same_seed_gives_the_same_water_and_the_lambda_round_trips() {
    let p = harsh(with_water(
        rich_of_size(Size::Medium),
        LayerPhase::After,
        HeightStyle::Hills,
    ));
    let a = serde_json::to_value(generate(5, &p).environment).unwrap();
    let b = serde_json::to_value(generate(5, &p).environment).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["environment"]["water"]["enabled"], true);
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, p.environment);
    let off = serde_json::to_value(rich_of_size(Size::Medium)).unwrap();
    assert!(off.get("environment").is_none());
}

#[test]
fn water_needs_the_ground_and_sane_settings() {
    let mut p = params_of_size(Size::Small);
    p.environment.water.enabled = true;
    assert!(p.intent_error().is_some(), "water without height");
    p.environment.height.enabled = true;
    assert!(p.intent_error().is_none());
    p.environment.water.sources = 0;
    assert!(p.intent_error().is_some());
    p.environment.water.sources = 2;
    p.environment.water.flow = -1.0;
    assert!(p.intent_error().is_some());
    p.environment.water.flow = 30.0;
    p.environment.water.reach = 0.0;
    assert!(p.intent_error().is_some());
}
