use tapiti_core::{
    environment::{light::transmission, placement::weights, Placement},
    generate::{generate, Node},
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn with_light(mut p: Params) -> Params {
    p.environment.light.enabled = true;
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

fn node_of(floor: &tapiti_core::generate::Floor, room: usize) -> usize {
    floor
        .nodes
        .iter()
        .position(|n| matches!(n, Node::Room { room: r } if *r == room))
        .unwrap()
}

#[test]
fn light_moves_no_room_and_leaves_the_other_layers_alone() {
    for (name, plain) in lambdas() {
        let mut all = plain.clone();
        all.environment.height.enabled = true;
        all.environment.water.enabled = true;
        all.environment.climate.enabled = true;
        let mut lit = all.clone();
        lit.environment.light.enabled = true;
        lit.environment.light.torches = 4;
        for seed in 1..=6 {
            assert_eq!(bare(seed, &all), bare(seed, &lit), "{name} seed {seed}");
            let (a, b) = (
                generate(seed, &all).environment.unwrap(),
                generate(seed, &lit).environment.unwrap(),
            );
            assert_eq!(
                (a.height, a.water, a.climate),
                (b.height, b.water, b.climate),
                "{name} seed {seed}"
            );
        }
    }
}

#[test]
fn no_node_is_dimmer_than_a_neighbour_lets_through_and_the_validator_agrees() {
    for (name, plain) in lambdas() {
        for time in [0.0, 9.0, 12.0, 20.0] {
            for placement in [
                Placement::Uniform,
                Placement::NearEntrance,
                Placement::FarFromEntrance,
                Placement::MainPath,
                Placement::DeadEnds,
                Placement::HighGround,
                Placement::LowGround,
            ] {
                let mut p = with_light(plain.clone());
                p.environment.height.enabled =
                    placement == Placement::HighGround || placement == Placement::LowGround;
                p.environment.light.time = time;
                p.environment.light.torches = 5;
                p.environment.light.openings = 2;
                p.environment.light.placement = placement;
                for seed in 1..=3 {
                    let s = stage(seed, &p, 1);
                    let tag = format!("{name} hour {time} {placement:?} seed {seed}");
                    assert!(
                        !s.violations.iter().any(|v| v.starts_with('E')),
                        "{tag}: {:?}",
                        s.violations
                    );
                    let field = s.d.environment.as_ref().unwrap().light.as_ref().unwrap();
                    assert!(
                        field.intensity.iter().all(|v| (0.0..=1.0).contains(v)),
                        "{tag}"
                    );
                    for e in &s.d.edges {
                        let t = transmission(
                            &p.environment.light,
                            tapiti_core::environment::height::edge_length(&s.d, e),
                        );
                        assert!(
                            field.intensity[e.v] >= field.intensity[e.u] * t - 1e-9,
                            "{tag}"
                        );
                    }
                    let m = metrics_of(&s.d).light.unwrap();
                    assert!(m.darkest <= m.mean && m.mean <= m.brightest, "{tag}");
                }
            }
        }
    }
}

#[test]
fn at_night_only_the_direct_sources_give_light_and_at_noon_the_sun_does() {
    let mut p = with_light(rich_of_size(Size::Medium));
    p.environment.light.time = 0.0;
    for seed in 1..=4 {
        let f = generate(seed, &p);
        let l = f.environment.as_ref().unwrap().light.as_ref().unwrap();
        assert!(
            l.intensity.iter().all(|v| *v == 0.0),
            "seed {seed}: light at midnight with no direct source"
        );
    }
    p.environment.light.time = 12.0;
    p.environment.light.daylight = 0.9;
    for seed in 1..=4 {
        let f = generate(seed, &p);
        let l = f.environment.as_ref().unwrap().light.as_ref().unwrap();
        let entrance = node_of(&f, f.entrance.unwrap());
        assert!((l.intensity[entrance] - 0.9).abs() < 1e-12, "seed {seed}");
    }
    p.environment.light.time = 0.0;
    p.environment.light.torches = 3;
    p.environment.light.torch_power = 0.6;
    for seed in 1..=4 {
        let f = generate(seed, &p);
        let l = f.environment.as_ref().unwrap().light.as_ref().unwrap();
        assert_eq!(l.torches.len(), 3);
        for &t in &l.torches {
            assert!(
                (l.intensity[t] - 0.6).abs() < 1e-12 || l.intensity[t] > 0.6,
                "seed {seed}"
            );
        }
    }
}

#[test]
fn doors_that_lose_less_light_more_of_the_floor() {
    let mut p = with_light(rich_of_size(Size::Medium));
    p.environment.light.torches = 2;
    let (mut dim, mut bright) = (0.0, 0.0);
    for seed in 1..=6 {
        for (loss, total) in [(0.4, &mut dim), (0.95, &mut bright)] {
            p.environment.light.door_loss = loss;
            let f = generate(seed, &p);
            *total += f
                .environment
                .as_ref()
                .unwrap()
                .light
                .as_ref()
                .unwrap()
                .intensity
                .iter()
                .sum::<f64>();
        }
    }
    assert!(bright > dim, "{bright} <= {dim}");
}

#[test]
fn each_placement_puts_sources_where_it_says() {
    let p = rich_of_size(Size::Large);
    let (mut near, mut far, mut n_near, mut n_far) = (0.0, 0.0, 0.0, 0.0);
    for seed in 1..=10 {
        let f = generate(seed, &p);
        let hops = tapiti_core::environment::height::hops_from_entrance(&f);
        let mut degree = vec![0usize; f.nodes.len()];
        for e in &f.edges {
            degree[e.u] += 1;
            degree[e.v] += 1;
        }
        let mut rng = tapiti_core::environment::layer_rng(seed, 7);
        for i in tapiti_core::environment::placement::place(
            &f,
            None,
            Placement::DeadEnds,
            4,
            &[],
            &mut rng,
        ) {
            // Fall back to anywhere only when the floor has no dead end.
            if degree.contains(&1) {
                assert_eq!(degree[i], 1, "seed {seed}: not a dead end");
            }
        }
        for i in tapiti_core::environment::placement::place(
            &f,
            None,
            Placement::MainPath,
            4,
            &[],
            &mut rng,
        ) {
            if !f.main_path.is_empty() {
                assert!(
                    f.main_path.contains(&i),
                    "seed {seed}: not on the main path"
                );
            }
        }
        for _ in 0..20 {
            for i in tapiti_core::environment::placement::place(
                &f,
                None,
                Placement::NearEntrance,
                2,
                &[],
                &mut rng,
            ) {
                near += hops[i] as f64;
                n_near += 1.0;
            }
            for i in tapiti_core::environment::placement::place(
                &f,
                None,
                Placement::FarFromEntrance,
                2,
                &[],
                &mut rng,
            ) {
                far += hops[i] as f64;
                n_far += 1.0;
            }
        }
        let w = weights(&f, None, Placement::Uniform);
        assert!(w.iter().all(|v| *v == 0.0 || *v == 1.0));
    }
    assert!(
        near / n_near < far / n_far,
        "near the entrance is not nearer than far from it"
    );
}

#[test]
fn the_same_seed_gives_the_same_light_and_the_lambda_round_trips() {
    let mut p = with_light(rich_of_size(Size::Medium));
    p.environment.light.torches = 3;
    let a = serde_json::to_value(generate(5, &p).environment).unwrap();
    let b = serde_json::to_value(generate(5, &p).environment).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["environment"]["light"]["torches"], 3);
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, p.environment);
}

#[test]
fn light_names_what_is_wrong() {
    let p = with_light(params_of_size(Size::Small));
    assert!(p.intent_error().is_none(), "light needs no other layer");
    let bad: [fn(&mut Params); 6] = [
        |p| p.environment.light.time = 25.0,
        |p| p.environment.light.daylight = 1.5,
        |p| p.environment.light.torch_power = -0.1,
        |p| p.environment.light.door_loss = 2.0,
        |p| p.environment.light.range = 0.0,
        |p| p.environment.light.torches = 300,
    ];
    for change in bad {
        let mut q = p.clone();
        change(&mut q);
        assert!(q.intent_error().is_some());
    }
}
