use fixed::traits::Fixed;
use num::Zero;

use crate::{
    math::real::{Base, Real},
    r,
};

pub fn sqrt(a: Real) -> Real {
    const ITERS: u8 = 6;

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
    const ITERS: u8 = 7;

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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use dashu_float::{DBig, FBig, ops::Abs};
    use nalgebra::ComplexField;

    use crate::math::real::Real;

    macro_rules! newton_test {
        ($f:ident, $ff:expr, $points:literal, [$from:literal, $to:literal]) => {
            #[test]
            fn $f() {
                const POINTS: u32 = $points;
                const FROM: f64 = $from;
                const TO: f64 = $to;
                let max_error: DBig = DBig::from_str("1e-22").unwrap();

                for i in 0..POINTS {
                    let t = FROM + i as f64 * (TO - FROM) / POINTS as f64;
                    let x = Real::from(t);

                    let x_f = DBig::from_str(&format!("{:.86}", x)).unwrap();
                    let newton = DBig::from_str(&format!("{:.86}", x.$f())).unwrap();
                    let f = ($ff)(x_f.with_precision(Real::FRAC_NBITS as usize).value());

                    let abs_err = (newton.clone() - f.clone()).abs();
                    let err = abs_err / f.clone().abs().max(DBig::ONE);

                    if err > max_error {
                        panic!(
                            "{f} excedeed max err with x = {}, NEWTON {f}(x) = {:e}, DBig {f}(x) = {:e}, err = {:e}",
                            x, newton, f, err, f=stringify!($f),
                        )
                    }
                }
            }
        };
    }

    newton_test!(sqrt, |x: DBig| x.sqrt(), 1000, [0.00000001, 1e11]);
    newton_test!(cbrt, |x: DBig| x.nth_root(3), 1000, [0.00000001, 1e11]);
}
