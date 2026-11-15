//! Reevaluating the environment layers on a finished floor gives the floor `generate` draws for the new Λ.

use tapiti_core::{
    environment::{reevaluate, EventSpec, Placement},
    generate::{generate, Floor},
    params::{params_of_size, rich_of_size, Layout, LevelsSpec, Params, Size, TopologyProfile},
};

fn clockless(f: &Floor) -> serde_json::Value {
    let mut v = serde_json::to_value(f).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    v
}

fn everything(mut p: Params) -> Params {
    p.environment.height.enabled = true;
    p.environment.water.enabled = true;
    p.environment.climate.enabled = true;
    p.environment.climate.season = 0.7;
    p.environment.light.enabled = true;
    p.environment.light.time = 9.0;
    p.environment.light.torches = 3;
    p.environment.events.enabled = true;
    p.environment.events.list = vec![EventSpec {
        name: "camp".into(),
        count: 3,
        placement: Placement::DeadEnds,
        ..EventSpec::default()
    }];
    p
}

fn lambdas() -> Vec<(&'static str, Params)> {
    let mut growth = rich_of_size(Size::Medium);
    growth.layout = Layout::Growth;
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    let mut ring = params_of_size(Size::Medium);
    ring.apply_intent(TopologyProfile::Ring);
    let mut tower = rich_of_size(Size::Medium);
    tower.layout = Layout::Growth;
    tower.levels = LevelsSpec {
        count: 2,
        ..LevelsSpec::default()
    };
    vec![
        ("grid", rich_of_size(Size::Medium)),
        ("growth", growth),
        ("bsp", bsp),
        ("ring", ring),
        ("two levels", tower),
    ]
}

#[test]
fn turning_the_layers_on_and_off_on_a_finished_floor_matches_a_new_generation() {
    for (name, off) in lambdas() {
        let on = everything(off.clone());
        for seed in 1..=4 {
            let mut floor = generate(seed, &off);
            reevaluate(&mut floor, &on);
            assert_eq!(
                clockless(&floor),
                clockless(&generate(seed, &on)),
                "{name} seed {seed}: on"
            );
            reevaluate(&mut floor, &off);
            assert_eq!(
                clockless(&floor),
                clockless(&generate(seed, &off)),
                "{name} seed {seed}: off again"
            );
        }
    }
}

#[test]
fn changing_one_layer_setting_matches_a_new_generation() {
    for (name, base) in lambdas() {
        let before = everything(base);
        let mut after = before.clone();
        after.environment.light.time = 0.0;
        after.environment.water.flow = 200.0;
        after.environment.climate.season = -1.0;
        after.environment.height.relief = 300.0;
        after.environment.events.list[0].count = 5;
        for seed in 1..=3 {
            let mut floor = generate(seed, &before);
            reevaluate(&mut floor, &after);
            assert_eq!(
                clockless(&floor),
                clockless(&generate(seed, &after)),
                "{name} seed {seed}"
            );
        }
    }
}
