//! One JSON line per case: the Λ and what the WebAssembly module's `stage_json` answers for it (without the clock readings), to
//! compare the native build with the module: `scripts/wasm-parity.sh`.
use serde_json::json;
#[path = "../tests/cases/mod.rs"]
mod cases;

use tapiti_core::{metrics::metrics_of, stage::stage};

fn main() {
    for (name, p) in cases::cases() {
        for seed in 1..=3 {
            let s = stage(seed, &p, 8);
            let mut floor = serde_json::to_value(&s.d).unwrap();
            floor.as_object_mut().unwrap().remove("phases");
            println!(
                "{}",
                json!({
                    "name": name,
                    "seed": seed,
                    "params": p,
                    "answer": {
                        "floor": floor,
                        "violations": s.violations,
                        "draws": s.draws,
                        "metrics": metrics_of(&s.d),
                    },
                })
            );
        }
    }
}
