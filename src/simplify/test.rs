use std::time::{self, Instant};

use crate::{
    expr::ops::{cos, cosh, ln, log, sinh},
    simplify::{Simplify, SimplifyContext, normal::Normalize},
    symbol::Symbol,
    symbols,
};

#[test]
fn factoring() {
    symbols!(x, y, z);

    let expr = (x * y * 3) + (6 * (y ^ 2));
    let simp = expr.simplify(&mut SimplifyContext::new());

    let target = x * ((ln(x) * (1 + z)) + y);
    assert_eq!(simp, target.normalize(true), "failed: {} vs {}", simp, target)
}
