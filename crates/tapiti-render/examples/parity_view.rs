//! One JSON line per case: the Λ, the seed and the 3D fingerprint of the floor, to compare the native build with the WebAssembly
//! module's `view_digest`: `scripts/wasm-parity.sh`.
use serde_json::json;
use tapiti_core::generate::generate;
use tapiti_render::digest;

#[path = "../tests/cases/mod.rs"]
mod cases;

fn main() {
    for (name, p) in cases::cases() {
        for seed in 1..=2 {
            let [m, a, b] = digest::of(&generate(seed, &p));
            println!(
                "{}",
                json!({ "name": name, "seed": seed, "params": p, "digest": format!("{m:016x} {a:016x} {b:016x}") })
            );
        }
    }
}
