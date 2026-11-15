use tapiti_core::{
    generate::generate,
    metrics::metrics_of,
    params::{params_of_size, Layout, Params, Size, TopologyProfile},
    stage::stage,
};

#[test]
fn spine_intent_reserves_a_route_dead_ends_and_returns() {
    for size in Size::ALL {
        let mut p = params_of_size(size);
        p.apply_intent(TopologyProfile::SpineWithDetours);
        let mut first = 0;
        let mut valid = 0;
        for seed in 1..=20 {
            let s = stage(seed, &p, 8);
            if s.violations.is_empty() {
                valid += 1;
                first += usize::from(s.draws == 1);
                let m = metrics_of(&s.d);
                assert!(m.hops_to_exit.unwrap() >= p.exit_min_hops as f64);
                assert!(m.side_branches >= 1);
                assert!(m.deepest_branch >= 1);
                assert!(m.returning_detours >= 1);
                assert_eq!(s.d.main_path.len(), p.exit_min_hops as usize + 1);
            } else {
                eprintln!(
                    "{} seed {seed}: {:?}; {:?}",
                    size.name(),
                    s.violations,
                    s.d.log
                );
            }
        }
        println!("{}: first={first}/20 valid={valid}/20", size.name());
        assert_eq!(valid, 20, "{}", size.name());
    }
}

#[test]
fn intent_round_trips_and_preserves_seed_determinism() {
    let mut p = params_of_size(Size::Medium);
    p.apply_intent(TopologyProfile::SpineWithDetours);
    let json = serde_json::to_value(&p).unwrap();
    let restored = serde_json::from_value(json.clone()).unwrap();
    let without_clock = |p| {
        let mut floor = serde_json::to_value(generate(7, p)).unwrap();
        floor.as_object_mut().unwrap().remove("phases");
        floor
    };
    assert_eq!(without_clock(&p), without_clock(&restored));
    assert_eq!(json["topology"]["profile"], "spineWithDetours");
    let old = serde_json::to_value(params_of_size(Size::Medium)).unwrap();
    assert!(old["topology"].get("minDetours").is_none());
}

#[test]
fn intent_selection_makes_compatible_settings_explicit() {
    let mut p = params_of_size(Size::Small);
    p.topology.profile = TopologyProfile::RedundantExit;
    assert!(p.intent_error().is_some());
    p.apply_intent(TopologyProfile::RedundantExit);
    assert_eq!(p.exit_doors, 2);
    assert!(p.intent_error().is_none());
    p.apply_intent(TopologyProfile::SpineWithDetours);
    assert_eq!(p.layout, Layout::Growth);
    assert!(p.intent_error().is_none());
    p.growth.islands = 2;
    assert!(p.intent_error().is_some());
}

#[test]
fn ring_closes_round_an_empty_court_with_two_routes_to_the_exit() {
    for size in Size::ALL {
        let mut p = params_of_size(size);
        p.apply_intent(TopologyProfile::Ring);
        assert!(p.intent_error().is_none(), "{}", size.name());
        let (centre, radii) = p.ring_ellipse();
        let (mut first, mut valid) = (0, 0);
        for seed in 1..=20 {
            let s = stage(seed, &p, 8);
            if !s.violations.is_empty() {
                eprintln!(
                    "{} seed {seed}: {:?}; {:?}",
                    size.name(),
                    s.violations,
                    s.d.log
                );
                continue;
            }
            valid += 1;
            first += usize::from(s.draws == 1);
            let m = metrics_of(&s.d);
            assert_eq!(
                m.cycle_girth,
                Some(p.ring_rooms()),
                "{} seed {seed}",
                size.name()
            );
            assert!(m.hops_to_exit.unwrap() >= p.exit_min_hops as f64);
            for room in s.d.rooms.iter().filter(|r| r.alive) {
                let (x, y) = (
                    (room.center[0] - centre[0]) / (0.5 * radii[0]),
                    (room.center[1] - centre[1]) / (0.5 * radii[1]),
                );
                assert!(
                    x * x + y * y >= 1.0,
                    "{} seed {seed}: room in the court",
                    size.name()
                );
            }
        }
        println!("ring {}: first={first}/20 valid={valid}/20", size.name());
        assert_eq!(valid, 20, "{}", size.name());
    }
}

#[test]
fn ring_intent_names_what_it_needs() {
    let mut p = params_of_size(Size::Medium);
    p.apply_intent(TopologyProfile::Ring);
    assert_eq!(p.exit_doors, 2);
    assert!(p.intent_error().is_none());
    p.growth.start = tapiti_core::params::GrowthStart::Center;
    assert!(p.intent_error().is_some());
    p.apply_intent(TopologyProfile::Ring);
    p.exit_min_hops = 40;
    assert!(p.intent_error().is_some());
}

#[test]
fn hub_and_wings_puts_a_hall_at_the_centre_with_wings_of_different_depths() {
    for size in Size::ALL {
        let mut p = params_of_size(size);
        p.apply_intent(TopologyProfile::HubAndWings);
        assert!(p.intent_error().is_none(), "{}", size.name());
        let (mut first, mut valid) = (0, 0);
        for seed in 1..=20 {
            let s = stage(seed, &p, 8);
            if !s.violations.is_empty() {
                eprintln!(
                    "{} seed {seed}: {:?}; {:?}",
                    size.name(),
                    s.violations,
                    s.d.log
                );
                continue;
            }
            valid += 1;
            first += usize::from(s.draws == 1);
            let m = metrics_of(&s.d);
            assert!(m.hops_to_exit.unwrap() >= p.exit_min_hops as f64);
            let degree = tapiti_core::topology::max_degree(s.d.nodes.len(), &s.d.edges);
            assert!(
                degree >= p.hub_wing_depths().len(),
                "{} seed {seed}: hub degree {degree}",
                size.name()
            );
        }
        println!("hub {}: first={first}/20 valid={valid}/20", size.name());
        assert_eq!(valid, 20, "{}", size.name());
    }
}

/// Layouts a zone can be set on: the grid, Growth, and Growth with the binary partition.
fn zone_layouts(size: Size, zones: usize, bottleneck: bool) -> Vec<(&'static str, Params)> {
    let mut grid = params_of_size(size);
    grid.set_zones(zones, false);
    let mut growth = params_of_size(size);
    growth.layout = Layout::Growth;
    growth.set_zones(zones, bottleneck);
    let mut bsp = params_of_size(size);
    bsp.layout = Layout::Bsp;
    bsp.set_zones(zones, bottleneck);
    vec![("grid", grid), ("growth", growth), ("bsp", bsp)]
}

#[test]
fn zones_style_every_layout_and_fill_every_strip() {
    for size in [Size::Medium, Size::Large, Size::Huge] {
        for zones in [2, 3] {
            for (name, p) in zone_layouts(size, zones, false) {
                assert!(p.intent_error().is_none(), "{name} {}", size.name());
                let n = p.zone_count();
                let (mut valid, mut styled) = (0, 0);
                for seed in 1..=20 {
                    let s = stage(seed, &p, 8);
                    if !s.violations.is_empty() {
                        continue;
                    }
                    valid += 1;
                    // The favoured class of each zone, counted among its rooms.
                    let mut tally = vec![[0usize; 5]; n];
                    for r in s.d.rooms.iter().filter(|r| r.alive && !r.leaf) {
                        tally[p.zone_of(r.center)][r.cls as usize] += 1;
                    }
                    let top: Vec<usize> = tally
                        .iter()
                        .map(|t| (0..5).max_by_key(|c| t[*c]).unwrap())
                        .collect();
                    styled += usize::from(
                        tally.iter().all(|t| t.iter().sum::<usize>() >= 2)
                            && top.windows(2).any(|w| w[0] != w[1]),
                    );
                }
                println!(
                    "zones {zones} {name} {}: valid={valid}/20 styled={styled}/20",
                    size.name()
                );
                assert!(valid >= 18, "{name} {} {zones}: {valid}", size.name());
                assert!(
                    styled >= 12,
                    "{name} {} {zones}: styled {styled}",
                    size.name()
                );
            }
        }
    }
}

#[test]
fn zone_bottlenecks_join_neighbours_by_one_corridor_in_growth_and_bsp() {
    for size in [Size::Medium, Size::Large, Size::Huge] {
        for zones in [2, 3] {
            for (name, p) in zone_layouts(size, zones, true).into_iter().skip(1) {
                assert!(p.intent_error().is_none(), "{name} {}", size.name());
                let (mut valid, mut single) = (0, 0);
                for seed in 1..=20 {
                    let s = stage(seed, &p, 8);
                    if !s.violations.is_empty() {
                        continue;
                    }
                    valid += 1;
                    // Corridors whose ends stand in different zones: one per boundary, none more.
                    let node_zone = |n: usize| match s.d.nodes[n] {
                        tapiti_core::generate::Node::Room { room } => {
                            Some(p.zone_of(s.d.rooms[room].center))
                        }
                        _ => None,
                    };
                    let mut crossing = vec![0usize; p.zone_count()];
                    for e in &s.d.edges {
                        if let (Some(a), Some(b)) = (node_zone(e.u), node_zone(e.v)) {
                            if a != b {
                                crossing[a.min(b)] += 1;
                            }
                        }
                    }
                    single += usize::from(crossing[..p.zone_count() - 1].iter().all(|c| *c == 1));
                }
                println!(
                    "bottleneck {zones} {name} {}: valid={valid}/20 single={single}/20",
                    size.name()
                );
                assert!(valid >= 18, "{name} {} {zones}: {valid}", size.name());
                assert!(
                    single >= valid - 2,
                    "{name} {} {zones}: single {single} of {valid}",
                    size.name()
                );
            }
        }
    }
}

#[test]
fn zones_say_what_they_cannot_do() {
    let mut p = params_of_size(Size::Medium);
    p.set_zones(2, true);
    assert!(
        p.intent_error().is_some(),
        "the grid cannot join zones by one corridor"
    );
    p.layout = Layout::Growth;
    assert!(p.intent_error().is_none());
    p.apply_intent(TopologyProfile::Ring);
    assert!(p.intent_error().is_some(), "a ring crosses every boundary");
    p.zones.bottleneck = false;
    assert!(p.intent_error().is_none(), "style only is fine");
    let mut small = params_of_size(Size::Small);
    small.set_zones(4, false);
    assert!(small.intent_error().is_some());
    let off = serde_json::to_value(params_of_size(Size::Medium)).unwrap();
    assert!(off.get("zones").is_none());
}

#[test]
fn bsp_puts_one_room_in_each_leaf_and_joins_them() {
    for size in Size::ALL {
        let mut p = params_of_size(size);
        p.layout = Layout::Bsp;
        assert!(p.intent_error().is_none(), "{}", size.name());
        let (mut valid, mut first) = (0, 0);
        for seed in 1..=20 {
            let s = stage(seed, &p, 8);
            if !s.violations.is_empty() {
                eprintln!(
                    "{} seed {seed}: {:?}; {:?}",
                    size.name(),
                    s.violations,
                    s.d.log
                );
                continue;
            }
            valid += 1;
            first += usize::from(s.draws == 1);
            let m = metrics_of(&s.d);
            assert!(s.d.rooms.iter().filter(|r| r.alive && !r.leaf).count() >= 2);
            assert!(m.hops_to_exit.unwrap() >= p.exit_min_hops as f64);
        }
        println!("bsp {}: first={first}/20 valid={valid}/20", size.name());
        assert!(valid >= 18, "{}: {valid}", size.name());
    }
}

#[test]
fn ring_and_hub_take_zone_styles_only() {
    for profile in [TopologyProfile::Ring, TopologyProfile::HubAndWings] {
        let mut p = params_of_size(Size::Large);
        p.apply_intent(profile);
        p.set_zones(2, false);
        assert!(p.intent_error().is_none(), "{profile:?}");
        let valid = (1..=20)
            .filter(|seed| stage(*seed, &p, 8).violations.is_empty())
            .count();
        println!("{profile:?} with zone styles: valid={valid}/20");
        assert!(valid >= 18, "{profile:?}: {valid}");
    }
}

#[test]
fn layouts_say_which_intents_they_run() {
    use tapiti_core::params::Layout;
    for profile in [
        TopologyProfile::SpineWithDetours,
        TopologyProfile::Ring,
        TopologyProfile::HubAndWings,
    ] {
        assert!(Layout::Growth.allows(profile));
        assert!(!Layout::Grid.allows(profile) && !Layout::Bsp.allows(profile));
        let mut p = params_of_size(Size::Medium);
        p.apply_intent(profile);
        p.layout = Layout::Bsp;
        assert!(
            p.intent_error().is_some(),
            "{profile:?} cannot run in a binary partition"
        );
    }
    for profile in [
        TopologyProfile::Classic,
        TopologyProfile::Backbone,
        TopologyProfile::Hub,
        TopologyProfile::RedundantExit,
        TopologyProfile::LongLoop,
    ] {
        assert!(Layout::Grid.allows(profile) && Layout::Bsp.allows(profile));
    }
    assert!(Layout::Bsp.places_freely() && !Layout::Grid.places_freely());
}
