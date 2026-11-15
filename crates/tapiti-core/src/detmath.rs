//! `sin`, `cos`, `exp` and `ln` from one pure-Rust implementation (`libm`) on every target. The standard library asks the platform's
//! math library, whose last digit differs between native builds and WebAssembly, and a last digit is enough to move a room's corner:
//! a seed and a Λ must draw the same stage everywhere. Square roots, sums and products are exact in IEEE 754 and need no help.

pub trait DetMath {
    fn det_sin(self) -> f64;
    fn det_cos(self) -> f64;
    fn det_exp(self) -> f64;
    fn det_ln(self) -> f64;
}

impl DetMath for f64 {
    fn det_sin(self) -> f64 {
        libm::sin(self)
    }
    fn det_cos(self) -> f64 {
        libm::cos(self)
    }
    fn det_exp(self) -> f64 {
        libm::exp(self)
    }
    fn det_ln(self) -> f64 {
        libm::log(self)
    }
}
