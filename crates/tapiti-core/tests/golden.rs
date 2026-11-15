//! Golden hashes: a seed and a Λ must draw the same floor on every platform and in every build. Each line of `golden.txt` is
//! the FNV-1a hash of the serialized floor (without the clock readings) for one named Λ and seed. A change that is meant to alter
//! the output says so in its commit and regenerates the file: `GOLDEN_UPDATE=1 cargo test -p tapiti-core --test golden`.

use tapiti_core::{
    generate::generate,
    params::{params_of_size, rich_of_size, Layout, Params, Size, TopologyProfile},
};

fn cases() -> Vec<(String, Params)> {
    let mut out = Vec::new();
    for size in [Size::Small, Size::Medium] {
        out.push((format!("grid-rich-{}", size.name()), rich_of_size(size)));
        let mut growth = rich_of_size(size);
        growth.layout = Layout::Growth;
        out.push((format!("growth-rich-{}", size.name()), growth));
    }
    let mut bsp = rich_of_size(Size::Medium);
    bsp.layout = Layout::Bsp;
    out.push(("bsp-rich-medium".into(), bsp));
    for (name, profile) in [
        ("backbone", TopologyProfile::Backbone),
        ("hub", TopologyProfile::Hub),
        ("redundant-exit", TopologyProfile::RedundantExit),
        ("long-loop", TopologyProfile::LongLoop),
        ("spine", TopologyProfile::SpineWithDetours),
        ("ring", TopologyProfile::Ring),
        ("hub-and-wings", TopologyProfile::HubAndWings),
    ] {
        let mut p = params_of_size(Size::Medium);
        p.apply_intent(profile);
        out.push((format!("intent-{name}-medium"), p));
    }
    for (name, layout) in [
        ("grid", Layout::Grid),
        ("growth", Layout::Growth),
        ("bsp", Layout::Bsp),
    ] {
        let mut p = params_of_size(Size::Large);
        p.layout = layout;
        p.set_zones(3, layout != Layout::Grid);
        out.push((format!("zones-{name}-large"), p));
    }
    out
}

fn hash_of(seed: u32, p: &Params) -> u64 {
    let mut v = serde_json::to_value(generate(seed, p)).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    let mut h: u64 = 0xcbf29ce484222325;
    for b in serde_json::to_string(&v).unwrap().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

#[test]
fn floors_match_the_checked_in_hashes() {
    let mut lines = Vec::new();
    for (name, p) in cases() {
        for seed in 1..=3 {
            lines.push(format!("{name} seed {seed}: {:016x}", hash_of(seed, &p)));
        }
    }
    let now = lines.join("\n") + "\n";
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.txt");
    if std::env::var_os("GOLDEN_UPDATE").is_some() {
        std::fs::write(path, &now).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path)
        .expect("tests/golden.txt: run with GOLDEN_UPDATE=1 to create it");
    let diff: Vec<String> = now
        .lines()
        .zip(want.lines())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("now  {a}\nwant {b}"))
        .collect();
    assert!(
        diff.is_empty() && now.lines().count() == want.lines().count(),
        "the output changed:\n{}",
        diff.join("\n")
    );
}
