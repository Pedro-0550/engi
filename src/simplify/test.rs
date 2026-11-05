use std::time::{self, Instant};

use ordered_float::Pow;

use crate as engi;
use crate::expr::{cos, sin};
use crate::{
    // expr::ops::{cos, cosh, ln, log, sinh},
    // simplify::{Simplify, SimplifyContext, normal::Normalize},
    symbol::Symbol,
    symbols,
};

#[test]
fn simple() {
    symbols!(x);
    assert_eq!(
        (sin(x).pow(2) + cos(x).pow(2) + 2 * x + 3 * x).simplified(),
        1 + x * 5
    );
}
