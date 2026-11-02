use fixed::traits::Fixed;
use num::Zero;

use crate::{
    math::real::{Base, Real},
    r,
};

pub const ITERS: u8 = 8;

pub fn sqrt(a: Real) -> Real {
    if a.is_zero() {
        return Real::zero();
    }

    if a < r!(0) {
        panic!("Cannot take sqrt of a negative Real");
    }

    let p = 127 - a.leading_zeros() as i32;
    let e = p - Real::FRAC_NBITS as i32;
    let q = e.div_euclid(2);
    let mut x = Real::from_bits(1_i128 << (Base::FRAC_NBITS as i32 + q));

    for _ in 0..ITERS {
        x = (x + a / x) / r!(2)
    }

    x
}

pub fn cbrt(a: Real) -> Real {
    if a.is_zero() {
        return Real::zero();
    }

    let negative = a < Real::zero();
    let a = if negative { -a } else { a };

    let p = 127 - a.leading_zeros() as i32;
    let e = p - Real::FRAC_NBITS as i32;
    let q = e.div_euclid(3);
    let mut x = Real::from_bits(1_i128 << (Base::FRAC_NBITS as i32 + q));

    for _ in 0..ITERS {
        x = (x + x + a / (x * x)) / r!(3)
    }

    if negative { -x } else { x }
}
