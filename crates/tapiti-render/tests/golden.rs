//! Golden fingerprints of the 3D look of a floor (see `digest`): `tests/golden.txt` has one line for each case and seed. A change meant to
//! alter how a floor is built or drawn regenerates it: `GOLDEN_UPDATE=1 cargo test -p tapiti-render --test golden`.

use tapiti_core::generate::generate;
use tapiti_render::digest;

mod cases;

#[test]
fn the_look_of_the_floors_matches_the_checked_in_fingerprints() {
    let mut lines = Vec::new();
    for (name, p) in cases::cases() {
        for seed in 1..=2 {
            let [m, a, b] = digest::of(&generate(seed, &p));
            lines.push(format!(
                "{name} seed {seed}: mesh {m:016x} outside {a:016x} inside {b:016x}"
            ));
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
        "the 3D look changed:\n{}",
        diff.join("\n")
    );
}
