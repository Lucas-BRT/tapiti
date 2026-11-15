use tapiti_core::{
    environment::{events::hops_from, EventSpec, Placement},
    generate::{generate, Node},
    metrics::metrics_of,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

fn event(name: &str, count: usize, placement: Placement) -> EventSpec {
    EventSpec {
        name: name.into(),
        count,
        placement,
        ..EventSpec::default()
    }
}

fn with_events(mut p: Params, list: Vec<EventSpec>) -> Params {
    p.environment.events.enabled = true;
    p.environment.events.list = list;
    p
}

fn lambdas() -> Vec<(&'static str, Params)> {
    let mut growth = rich_of_size(Size::Large);
    growth.layout = Layout::Growth;
    let mut bsp = rich_of_size(Size::Large);
    bsp.layout = Layout::Bsp;
    let mut spine = params_of_size(Size::Large);
    spine.apply_intent(TopologyProfile::SpineWithDetours);
    let mut ring = params_of_size(Size::Large);
    ring.apply_intent(TopologyProfile::Ring);
    vec![
        ("grid", rich_of_size(Size::Large)),
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

fn all_placements() -> Vec<EventSpec> {
    [
        Placement::Uniform,
        Placement::NearEntrance,
        Placement::FarFromEntrance,
        Placement::MainPath,
        Placement::DeadEnds,
        Placement::HighGround,
        Placement::LowGround,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, pl)| event(&format!("kind-{i}"), 3, pl))
    .collect()
}

#[test]
fn events_move_no_room_and_leave_the_other_layers_alone() {
    for (name, plain) in lambdas() {
        let mut all = plain.clone();
        all.environment.height.enabled = true;
        all.environment.light.enabled = true;
        let ev = with_events(all.clone(), all_placements());
        for seed in 1..=4 {
            assert_eq!(bare(seed, &all), bare(seed, &ev), "{name} seed {seed}");
            let (a, b) = (
                generate(seed, &all).environment.unwrap(),
                generate(seed, &ev).environment.unwrap(),
            );
            assert_eq!(
                (a.height, a.light),
                (b.height, b.light),
                "{name} seed {seed}"
            );
        }
    }
}

#[test]
fn the_validator_agrees_for_every_placement_exclusive_or_not_with_gaps() {
    for (name, plain) in lambdas() {
        for exclusive in [true, false] {
            for gap in [0, 2] {
                let mut list = all_placements();
                for e in &mut list {
                    e.min_gap = gap;
                }
                let mut p = with_events(plain.clone(), list);
                p.environment.height.enabled = true;
                p.environment.events.exclusive = exclusive;
                for seed in 1..=3 {
                    let s = stage(seed, &p, 1);
                    let tag = format!("{name} exclusive {exclusive} gap {gap} seed {seed}");
                    assert!(
                        !s.violations.iter().any(|v| v.starts_with('E')),
                        "{tag}: {:?}",
                        s.violations
                    );
                    let m = metrics_of(&s.d).events.unwrap();
                    assert_eq!(m.kinds, 7, "{tag}");
                    assert!(m.placed + m.missing == 21, "{tag}");
                    let field = s.d.environment.as_ref().unwrap().events.as_ref().unwrap();
                    for kind in &field.kinds {
                        for (k, &i) in kind.nodes.iter().enumerate() {
                            for &j in &kind.nodes[..k] {
                                assert!(
                                    gap == 0 || hops_from(&s.d, i)[j] > gap,
                                    "{tag}: {} too close",
                                    kind.name
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_placement_puts_its_events_where_it_says() {
    let p = with_events(
        rich_of_size(Size::Large),
        vec![
            event("trap", 4, Placement::DeadEnds),
            event("guard", 4, Placement::MainPath),
        ],
    );
    for seed in 1..=8 {
        let f = generate(seed, &p);
        let mut degree = vec![0usize; f.nodes.len()];
        for e in &f.edges {
            degree[e.u] += 1;
            degree[e.v] += 1;
        }
        let field = f.environment.as_ref().unwrap().events.as_ref().unwrap();
        if degree.contains(&1) {
            for &i in &field.kinds[0].nodes {
                assert_eq!(degree[i], 1, "seed {seed}: not a dead end");
            }
        }
        if !f.main_path.is_empty() {
            for &i in &field.kinds[1].nodes {
                assert!(
                    f.main_path.contains(&i),
                    "seed {seed}: not on the main path"
                );
            }
        }
    }
}

#[test]
fn events_keep_off_the_entrance_and_stay_in_their_zone() {
    let mut p = params_of_size(Size::Large);
    p.layout = Layout::Growth;
    p.set_zones(3, true);
    let mut e = event("camp", 3, Placement::Uniform);
    e.zone = Some(1);
    let p = with_events(p, vec![e, event("many", 256, Placement::Uniform)]);
    assert!(p.intent_error().is_none());
    for seed in 1..=6 {
        let f = generate(seed, &p);
        let field = f.environment.as_ref().unwrap().events.as_ref().unwrap();
        for &i in &field.kinds[0].nodes {
            let Node::Room { room } = f.nodes[i] else {
                panic!("an event on a junction")
            };
            assert_eq!(p.zone_of(f.rooms[room].center), 1, "seed {seed}");
        }
        let doors: Vec<usize> = [f.entrance, f.exit]
            .into_iter()
            .flatten()
            .map(|r| {
                f.nodes
                    .iter()
                    .position(|n| matches!(n, Node::Room { room } if *room == r))
                    .unwrap()
            })
            .collect();
        for kind in &field.kinds {
            assert!(
                kind.nodes.iter().all(|i| !doors.contains(i)),
                "seed {seed}: an event on the entrance or the exit"
            );
        }
        let m = metrics_of(&stage(seed, &p, 1).d).events.unwrap();
        assert!(m.missing > 0, "seed {seed}: 259 events always fit?");
    }
}

#[test]
fn the_same_seed_gives_the_same_events_and_the_list_round_trips() {
    let p = with_events(rich_of_size(Size::Medium), all_placements());
    let a = serde_json::to_value(generate(5, &p).environment).unwrap();
    let b = serde_json::to_value(generate(5, &p).environment).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(
        json["environment"]["events"]["list"][4]["placement"],
        "deadEnds"
    );
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, p.environment);
    assert!(serde_json::to_value(rich_of_size(Size::Medium))
        .unwrap()
        .get("environment")
        .is_none());
}

#[test]
fn events_name_what_is_wrong() {
    let p = with_events(
        params_of_size(Size::Small),
        vec![event("a", 1, Placement::Uniform)],
    );
    assert!(p.intent_error().is_none());
    let bad: [fn(&mut Params); 6] = [
        |p| p.environment.events.list[0].name = "  ".into(),
        |p| p.environment.events.list[0].name = "x".repeat(33),
        |p| {
            p.environment.events.list.push(EventSpec {
                name: "a".into(),
                ..EventSpec::default()
            })
        },
        |p| p.environment.events.list[0].count = 300,
        |p| p.environment.events.list[0].zone = Some(5),
        |p| {
            p.environment.events.list = (0..17)
                .map(|i| EventSpec {
                    name: format!("k{i}"),
                    ..EventSpec::default()
                })
                .collect()
        },
    ];
    for change in bad {
        let mut q = p.clone();
        change(&mut q);
        assert!(q.intent_error().is_some());
    }
}
