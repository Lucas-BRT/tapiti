use tapiti_core::{
    environment::{climate::depths, Climate, HeightStyle, LayerPhase},
    generate::{generate, Node},
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn with_climate(mut p: Params) -> Params {
    p.environment.height.enabled = true;
    p.environment.climate.enabled = true;
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
fn climate_moves_no_room_and_changes_neither_height_nor_water() {
    for (name, plain) in lambdas() {
        let mut wet = plain.clone();
        wet.environment.height.enabled = true;
        wet.environment.water.enabled = true;
        let mut both = wet.clone();
        both.environment.climate.enabled = true;
        for seed in 1..=6 {
            assert_eq!(bare(seed, &wet), bare(seed, &both), "{name} seed {seed}");
            let (a, b) = (
                generate(seed, &wet).environment.unwrap(),
                generate(seed, &both).environment.unwrap(),
            );
            assert_eq!(a.height, b.height, "{name} seed {seed}");
            assert_eq!(a.water, b.water, "{name} seed {seed}");
        }
    }
}

#[test]
fn every_temperature_is_the_formula_and_the_validator_agrees() {
    for (name, plain) in lambdas() {
        for climate in [Climate::Cold, Climate::Temperate, Climate::Hot] {
            for season in [-1.0, 0.0, 1.0] {
                for water in [false, true] {
                    for phase in [LayerPhase::After, LayerPhase::Before] {
                        let mut p = with_climate(plain.clone());
                        p.environment.height.phase = phase;
                        p.environment.height.style = HeightStyle::Hills;
                        p.environment.water.enabled = water;
                        p.environment.climate.climate = climate;
                        p.environment.climate.season = season;
                        p.environment.climate.openings = 2;
                        for seed in 1..=4 {
                            let s = stage(seed, &p, 1);
                            let tag = format!("{name} {climate:?} season {season} water {water} {phase:?} seed {seed}");
                            assert!(
                                !s.violations.iter().any(|v| v.starts_with('E')),
                                "{tag}: {:?}",
                                s.violations
                            );
                            let env = s.d.environment.as_ref().unwrap();
                            let field = env.climate.as_ref().unwrap();
                            assert!(
                                field.openness.iter().all(|o| (0.0..=1.0).contains(o)),
                                "{tag}"
                            );
                            assert!(field.celsius.iter().all(|t| t.is_finite()), "{tag}");
                            assert!(
                                field.openings.len() <= 3 && !field.openings.is_empty(),
                                "{tag}"
                            );
                            for &i in &field.openings {
                                assert_eq!(field.openness[i], 1.0, "{tag}: an opening is not open");
                            }
                            let m = metrics_of(&s.d).climate.expect("climate measures");
                            assert!(m.coldest <= m.mean && m.mean <= m.hottest, "{tag}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn without_a_season_or_water_the_floor_only_warms_with_depth() {
    let mut p = with_climate(rich_of_size(Size::Medium));
    p.environment.climate.season = 0.0;
    for seed in 1..=6 {
        let floor = generate(seed, &p);
        let env = floor.environment.as_ref().unwrap();
        let (height, climate) = (env.height.as_ref().unwrap(), env.climate.as_ref().unwrap());
        let depth = depths(&height.nodes);
        let (mean, _) = p.environment.climate.climate.values();
        for (i, (t, d)) in climate.celsius.iter().zip(&depth).enumerate() {
            let want = mean + p.environment.climate.gradient * d;
            assert!((t - want).abs() < 1e-9, "seed {seed} node {i}");
        }
        let (top, bottom) = (
            (0..depth.len())
                .min_by(|&a, &b| depth[a].total_cmp(&depth[b]))
                .unwrap(),
            (0..depth.len())
                .max_by(|&a, &b| depth[a].total_cmp(&depth[b]))
                .unwrap(),
        );
        assert!(
            climate.celsius[bottom] >= climate.celsius[top],
            "seed {seed}: the deepest node is colder than the highest"
        );
    }
}

#[test]
fn summer_warms_the_openings_and_humidity_softens_it() {
    let mut p = with_climate(rich_of_size(Size::Medium));
    p.environment.climate.climate = Climate::Temperate;
    p.environment.climate.season = 1.0;
    let mut dry = p.clone();
    dry.environment.water.enabled = false;
    let mut wet = p.clone();
    wet.environment.water.enabled = true;
    wet.environment.water.flow = 400.0;
    wet.environment.water.sources = 6;
    let mut softened = false;
    for seed in 1..=8 {
        let (d, w) = (generate(seed, &dry), generate(seed, &wet));
        let (dc, wc) = (
            d.environment.as_ref().unwrap().climate.as_ref().unwrap(),
            w.environment.as_ref().unwrap().climate.as_ref().unwrap(),
        );
        let humidity = &w
            .environment
            .as_ref()
            .unwrap()
            .water
            .as_ref()
            .unwrap()
            .humidity;
        for (i, ((dry_t, wet_t), u)) in dc.celsius.iter().zip(&wc.celsius).zip(humidity).enumerate()
        {
            // Summer only adds heat at an opening, and humidity only takes some of it back.
            assert!(*wet_t <= dry_t + 1e-9, "seed {seed} node {i}");
            softened |= *u > 0.5 && dc.openness[i] > 0.1 && *wet_t < dry_t - 1e-6;
        }
        if let Some(Node::Room { .. }) = d.nodes.first() {
            let entrance = d.entrance.unwrap();
            assert!(dc.openings.contains(
                &d.nodes
                    .iter()
                    .position(|n| matches!(n, Node::Room { room } if *room == entrance))
                    .unwrap()
            ));
        }
    }
    assert!(softened, "humidity never softened the surface's heat");
}

#[test]
fn the_same_seed_gives_the_same_climate_and_the_lambda_round_trips() {
    let p = with_climate(rich_of_size(Size::Medium));
    let a = serde_json::to_value(generate(5, &p).environment).unwrap();
    let b = serde_json::to_value(generate(5, &p).environment).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["environment"]["climate"]["enabled"], true);
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, p.environment);
}

#[test]
fn climate_needs_the_ground_and_sane_settings() {
    let mut p = params_of_size(Size::Small);
    p.environment.climate.enabled = true;
    assert!(p.intent_error().is_some(), "climate without height");
    p.environment.height.enabled = true;
    assert!(p.intent_error().is_none());
    let bad: [fn(&mut Params); 4] = [
        |p| p.environment.climate.season = 2.0,
        |p| p.environment.climate.reach = 0.0,
        |p| p.environment.climate.gradient = -1.0,
        |p| p.environment.climate.openings = 65,
    ];
    for change in bad {
        let mut q = p.clone();
        change(&mut q);
        assert!(q.intent_error().is_some());
    }
}
