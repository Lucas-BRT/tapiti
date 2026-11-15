//! Golden hashes: a seed and a Λ must draw the same floor on every platform and in every build. Each line of `golden.txt` is
//! the FNV-1a hash of the serialized floor (without the clock readings) for one named Λ and seed. A change that is meant to alter
//! the output says so in its commit and regenerates the file: `GOLDEN_UPDATE=1 cargo test -p tapiti-core --test golden`.

mod cases;

use tapiti_core::{generate::generate, params::Params};

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
    for (name, p) in cases::cases() {
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
