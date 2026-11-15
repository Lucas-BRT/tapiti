use tapiti_core::{
    environment::{HeightStyle, Placement},
    generate::{generate, level_floor, level_seed, Floor},
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, LevelsSpec, Params, Size, TopologyProfile},
    stage::stage,
};

fn stacked(mut p: Params, count: usize) -> Params {
    p.levels = LevelsSpec {
        count,
        ..LevelsSpec::default()
    };
    p
}

fn lambdas() -> Vec<(&'static str, Params)> {
    let mut growth = rich_of_size(Size::Medium);
    growth.layout = Layout::Growth;
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    let mut spine = params_of_size(Size::Medium);
    spine.apply_intent(TopologyProfile::SpineWithDetours);
    let mut ring = params_of_size(Size::Medium);
    ring.apply_intent(TopologyProfile::Ring);
    let mut hub = params_of_size(Size::Medium);
    hub.apply_intent(TopologyProfile::HubAndWings);
    let mut redundant = params_of_size(Size::Medium);
    redundant.apply_intent(TopologyProfile::RedundantExit);
    let mut long = rich_of_size(Size::Medium);
    long.topology.profile = TopologyProfile::LongLoop;
    vec![
        ("grid", rich_of_size(Size::Medium)),
        ("growth", growth),
        ("bsp", bsp),
        ("spine", spine),
        ("ring", ring),
        ("hub-and-wings", hub),
        ("redundant-exit", redundant),
        ("long-loop", long),
    ]
}

/// A floor as JSON without what only a whole stacked floor has or the clock changes.
fn plain(f: &Floor) -> serde_json::Value {
    let mut v = serde_json::to_value(f).unwrap();
    let o = v.as_object_mut().unwrap();
    for key in [
        "phases",
        "params",
        "environment",
        "stack",
        "log",
        "candidates",
        "pairs",
        "dropped",
        "betaMax",
        "betaTarget",
        "seed",
    ] {
        o.remove(key);
    }
    v
}

#[test]
fn one_level_is_the_floor_it_always_was() {
    let p = rich_of_size(Size::Medium);
    let one = stacked(p.clone(), 1);
    assert!(serde_json::to_value(&one).unwrap().get("levels").is_none());
    for seed in 1..=4 {
        assert_eq!(
            serde_json::to_value(generate(seed, &p)).unwrap()["rooms"],
            serde_json::to_value(generate(seed, &one)).unwrap()["rooms"]
        );
        assert!(generate(seed, &one).stack.is_none());
    }
}

#[test]
fn each_level_is_the_floor_its_own_seed_draws_and_nothing_leaks() {
    for (name, base) in lambdas() {
        for count in [2, 3] {
            let p = stacked(base.clone(), count);
            for seed in 1..=3 {
                let floor = generate(seed, &p);
                let mut q = p.clone();
                q.levels = LevelsSpec::default();
                for k in 0..count {
                    let alone = generate(level_seed(seed, k), &q);
                    assert_eq!(
                        plain(&level_floor(&floor, k)),
                        plain(&alone),
                        "{name} {count} levels seed {seed} level {k}"
                    );
                }
            }
        }
    }
}

#[test]
fn stairs_join_consecutive_levels_where_rooms_overlap_and_the_floor_validates() {
    let mut overlapping_levels = false;
    for (name, base) in lambdas() {
        for count in [2, 3] {
            let mut p = stacked(base.clone(), count);
            p.levels.stairs = 2;
            let mut valid = 0;
            for seed in 1..=8 {
                let s = stage(seed, &p, 8);
                if !s.violations.is_empty() {
                    continue;
                }
                valid += 1;
                let f = &s.d;
                let stairs: Vec<_> = f.edges.iter().filter(|e| e.stair).collect();
                assert!(
                    stairs.len() >= count - 1,
                    "{name} {count} levels seed {seed}"
                );
                for e in stairs {
                    let level = |n: usize| match f.nodes[n] {
                        tapiti_core::generate::Node::Room { room } => f.rooms[room].level,
                        _ => panic!("a stair ends in a junction"),
                    };
                    assert_eq!(
                        level(e.u).abs_diff(level(e.v)),
                        1,
                        "{name} seed {seed}: a stair skips a level"
                    );
                }
                for a in f.rooms.iter().filter(|r| r.level == 0) {
                    for b in f.rooms.iter().filter(|r| r.level == 1) {
                        overlapping_levels |= a.bounds.lo[0] < b.bounds.hi[0]
                            && b.bounds.lo[0] < a.bounds.hi[0]
                            && a.bounds.lo[1] < b.bounds.hi[1]
                            && b.bounds.lo[1] < a.bounds.hi[1];
                    }
                }
            }
            println!("{name}, {count} levels: valid {valid}/8");
            assert!(valid >= 4, "{name} {count} levels: {valid}/8");
        }
    }
    assert!(
        overlapping_levels,
        "no room of one level ever overlapped a room of the next: the test proves nothing"
    );
}

#[test]
fn every_layer_validates_on_a_stacked_floor_and_each_level_stands_a_storey_higher() {
    for (name, base) in lambdas().into_iter().take(5) {
        let mut p = stacked(base, 3);
        p.environment.height.enabled = true;
        p.environment.height.style = HeightStyle::Hills;
        p.environment.water.enabled = true;
        p.environment.climate.enabled = true;
        p.environment.light.enabled = true;
        p.environment.light.torches = 3;
        p.environment.events.enabled = true;
        p.environment.events.list = vec![tapiti_core::environment::EventSpec {
            name: "camp".into(),
            count: 4,
            placement: Placement::FarFromEntrance,
            ..Default::default()
        }];
        let mut checked = 0;
        for seed in 1..=8 {
            let s = stage(seed, &p, 8);
            if !s.violations.is_empty() {
                continue;
            }
            checked += 1;
            assert!(
                !s.violations.iter().any(|v| v.starts_with("E")),
                "{name} seed {seed}"
            );
            let env = s.d.environment.as_ref().unwrap();
            let height = env.height.as_ref().unwrap();
            let mean = |level: usize| {
                let hs: Vec<f64> =
                    s.d.nodes
                        .iter()
                        .zip(&height.nodes)
                        .filter_map(|(n, h)| match n {
                            tapiti_core::generate::Node::Room { room }
                                if s.d.rooms[*room].level == level && s.d.rooms[*room].alive =>
                            {
                                Some(*h)
                            }
                            _ => None,
                        })
                        .collect();
                hs.iter().sum::<f64>() / hs.len() as f64
            };
            assert!(
                mean(1) > mean(0) && mean(2) > mean(1),
                "{name} seed {seed}: the levels do not climb"
            );
            assert!(metrics_of(&s.d).height.is_some());
        }
        assert!(checked >= 3, "{name}: only {checked} valid stacked floors");
    }
}

#[test]
fn levels_name_what_is_wrong_and_round_trip() {
    let p = stacked(params_of_size(Size::Medium), 3);
    assert!(p.intent_error().is_none());
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["levels"]["count"], 3);
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.levels, p.levels);
    let bad: [fn(&mut Params); 6] = [
        |p| p.levels.count = 9,
        |p| p.levels.count = 0,
        |p| p.levels.stairs = 5,
        |p| p.levels.storey = 0.0,
        |p| p.exit_room = false,
        |p| {
            p.environment.height.enabled = true;
            p.environment.height.phase = tapiti_core::environment::LayerPhase::Before;
        },
    ];
    for change in bad {
        let mut q = p.clone();
        change(&mut q);
        assert!(q.intent_error().is_some());
    }
}
