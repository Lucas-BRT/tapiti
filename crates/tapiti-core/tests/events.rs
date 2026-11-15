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

use tapiti_core::environment::{Condition, ConditionField};

fn when(field: ConditionField, min: Option<f64>, max: Option<f64>) -> Vec<Condition> {
    vec![Condition { field, min, max }]
}

fn node_room(f: &tapiti_core::generate::Floor, i: usize) -> &tapiti_core::generate::Room {
    match f.nodes[i] {
        Node::Room { room } => &f.rooms[room],
        _ => panic!("an event on a junction"),
    }
}

#[test]
fn an_event_only_happens_where_its_conditions_hold() {
    let mut base = rich_of_size(Size::Large);
    base.layout = Layout::Growth;
    base.environment.height.enabled = true;
    base.environment.water.enabled = true;
    base.environment.water.sources = 8;
    base.environment.water.flow = 300.0;
    base.environment.climate.enabled = true;
    base.environment.climate.climate = tapiti_core::environment::Climate::Hot;
    base.environment.climate.season = 1.0;
    base.environment.light.enabled = true;
    base.environment.light.time = 0.0;
    base.environment.light.torches = 3;
    let e = |name: &str, c: Vec<Condition>| EventSpec {
        name: name.into(),
        count: 6,
        conditions: c,
        ..EventSpec::default()
    };
    let p = with_events(
        base,
        vec![
            e("dark", when(ConditionField::Light, None, Some(0.05))),
            e("damp", when(ConditionField::Humidity, Some(0.5), None)),
            e("hot", when(ConditionField::Temperature, Some(30.0), None)),
            e("high", when(ConditionField::Height, Some(100.0), None)),
            e("never", when(ConditionField::Light, Some(2.0), None)),
        ],
    );
    assert!(p.intent_error().is_none());
    let (mut dark, mut damp, mut hot, mut high) = (0, 0, 0, 0);
    for seed in 1..=6 {
        let s = stage(seed, &p, 1);
        assert!(
            !s.violations.iter().any(|v| v.starts_with("E5")),
            "seed {seed}: {:?}",
            s.violations
        );
        let env = s.d.environment.as_ref().unwrap();
        let (light, water, climate, height) = (
            env.light.as_ref().unwrap(),
            env.water.as_ref().unwrap(),
            env.climate.as_ref().unwrap(),
            env.height.as_ref().unwrap(),
        );
        let kinds = &env.events.as_ref().unwrap().kinds;
        for &i in &kinds[0].nodes {
            assert!(light.intensity[i] <= 0.05, "seed {seed}");
            dark += 1;
        }
        for &i in &kinds[1].nodes {
            assert!(water.humidity[i] >= 0.5, "seed {seed}");
            damp += 1;
        }
        for &i in &kinds[2].nodes {
            assert!(climate.celsius[i] >= 30.0, "seed {seed}");
            hot += 1;
        }
        for &i in &kinds[3].nodes {
            assert!(height.nodes[i] >= 100.0, "seed {seed}");
            high += 1;
        }
        assert!(
            kinds[4].nodes.is_empty(),
            "seed {seed}: a room with light of 2"
        );
        let m = metrics_of(&s.d).events.unwrap();
        assert!(m.missing >= 6, "seed {seed}");
        let _ = node_room(&s.d, kinds[0].nodes.first().copied().unwrap_or(0));
    }
    assert!(
        dark > 0 && damp > 0 && hot > 0 && high > 0,
        "a condition never found a room: {dark} {damp} {hot} {high}"
    );
}

#[test]
fn an_event_can_be_limited_to_a_level() {
    let mut p = rich_of_size(Size::Medium);
    p.layout = Layout::Growth;
    p.levels.count = 3;
    let mut e = event("lair", 4, Placement::Uniform);
    e.conditions = when(ConditionField::Level, Some(2.0), Some(2.0));
    let p = with_events(p, vec![e]);
    let mut seen = 0;
    for seed in 1..=6 {
        let s = stage(seed, &p, 8);
        // Three levels do not always find the overlaps their stairs need.
        if !s.violations.is_empty() {
            continue;
        }
        for &i in
            &s.d.environment
                .as_ref()
                .unwrap()
                .events
                .as_ref()
                .unwrap()
                .kinds[0]
                .nodes
        {
            assert_eq!(node_room(&s.d, i).level, 1, "seed {seed}");
            seen += 1;
        }
    }
    assert!(seen > 0);
}

#[test]
fn conditions_need_their_layers_and_sane_limits_and_round_trip() {
    let mut e = event("a", 1, Placement::Uniform);
    e.conditions = when(ConditionField::Light, None, Some(0.1));
    let p = with_events(params_of_size(Size::Small), vec![e.clone()]);
    assert!(
        p.intent_error().is_some(),
        "light condition without the light layer"
    );
    let mut on = p.clone();
    on.environment.light.enabled = true;
    assert!(on.intent_error().is_none());
    let mut backwards = on.clone();
    backwards.environment.events.list[0].conditions =
        when(ConditionField::Light, Some(0.5), Some(0.1));
    assert!(backwards.intent_error().is_some());
    let json = serde_json::to_value(&on).unwrap();
    assert_eq!(
        json["environment"]["events"]["list"][0]["conditions"][0]["field"],
        "light"
    );
    let back: Params = serde_json::from_value(json).unwrap();
    assert_eq!(back.environment, on.environment);
    let plain = serde_json::to_value(with_events(
        params_of_size(Size::Small),
        vec![event("a", 1, Placement::Uniform)],
    ))
    .unwrap();
    assert!(plain["environment"]["events"]["list"][0]
        .get("conditions")
        .is_none());
}
