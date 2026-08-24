use std::{
    fmt::{Display, Write},
    ops::*,
};

use float_eq::float_eq;
use num::{
    Complex, Float, Zero,
    complex::{Complex32, Complex64},
    pow::Pow,
};

use crate::{impl_assign_op, impl_binary_op};

pub const EQ_ABS_TOL: f64 = 1e-15;

pub const I: Scalar = Scalar(Complex::I);

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Scalar(pub Complex<f64>);

/* ---------------------------------- IMPLS --------------------------------- */

impl Scalar {
    pub fn is_integer(self) -> bool {
        float_eq!(self.im, 0.0, abs <= EQ_ABS_TOL)
            && float_eq!(self.re, self.re.round(), abs <= EQ_ABS_TOL)
    }

    pub fn is_real(self) -> bool {
        float_eq!(self.im, 0.0, abs <= EQ_ABS_TOL)
    }

    pub fn is_imag(self) -> bool {
        self.im.abs() > 0.0 && float_eq!(self.re, 0.0, abs <= EQ_ABS_TOL)
    }

    pub fn as_integer(self) -> Option<i64> {
        if self.is_integer() { Some(self.re as i64) } else { None }
    }

    pub fn as_real(self) -> Option<f64> {
        if self.is_real() { Some(self.re) } else { None }
    }

    pub fn as_imag(self) -> Option<f64> {
        if self.is_imag() { Some(self.re) } else { None }
    }
}

macro_rules! impl_scalar_from_real {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Scalar {
                fn from(value: $t) -> Self {
                    Scalar(Complex::new(value as f64, 0.0))
                }
            }
        )*
    };
}

impl_scalar_from_real!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

impl Deref for Scalar {
    type Target = Complex64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Complex64> for Scalar {
    fn from(value: Complex64) -> Self {
        Self(value)
    }
}

impl From<Complex32> for Scalar {
    fn from(value: Complex32) -> Self {
        Self(Complex::new(value.re as f64, value.im as f64))
    }
}

impl Pow<Scalar> for Scalar {
    type Output = Scalar;

    fn pow(self, rhs: Scalar) -> Self::Output {
        Scalar(self.0.powc(rhs.0))
    }
}

macro_rules! impl_scalar_ops {
    (
        $t0:ty, [$($ty:ty),+ $(,)?], $config:tt
    ) => {
        $(
            impl_binary_op!(Scalar, $t0, $ty, Add, add, |lhs: Scalar, rhs: Scalar| Scalar(lhs.0 + rhs.0), $config);
            impl_assign_op!($t0, $ty, AddAssign, add_assign, |lhs: &mut Scalar, rhs: Scalar| lhs.0 += rhs.0);

            impl_binary_op!(Scalar, $t0, $ty, Mul, mul, |lhs: Scalar, rhs: Scalar| Scalar(lhs.0 * rhs.0), $config);
            impl_assign_op!($t0, $ty, MulAssign, mul_assign, |lhs: &mut Scalar, rhs: Scalar| lhs.0 *= rhs.0);

            impl_binary_op!(Scalar, $t0, $ty, Div, div, |lhs: Scalar, rhs: Scalar| Scalar(lhs.0 / rhs.0), $config);
            impl_assign_op!($t0, $ty, DivAssign, div_assign, |lhs: &mut Scalar, rhs: Scalar| lhs.0 /= rhs.0);

            impl_binary_op!(Scalar, $t0, $ty, Sub, sub, |lhs: Scalar, rhs: Scalar| Scalar(lhs.0 - rhs.0), $config);
            impl_assign_op!($t0, $ty, SubAssign, sub_assign, |lhs: &mut Scalar, rhs: Scalar| lhs.0 -= rhs.0);

            impl_binary_op!(Scalar, $t0, $ty, BitXor, bitxor, |lhs: Scalar, rhs: Scalar| lhs.pow(rhs), $config);
        )+
    };
}

impl_scalar_ops!(
    Scalar,
    [f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, Complex64, Complex32],
    symmetrical
);

impl_scalar_ops!(Scalar, [Scalar], normal);

impl Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.0.re.is_zero(), self.0.im.is_zero()) {
            (true, true) => f.write_str("0"),
            (true, false) => {
                self.0.im.fmt(f)?;
                f.write_char('i')
            }
            (false, true) => self.0.re.fmt(f),
            (false, false) => self.0.fmt(f),
        }
    }
}

pub fn gcd_f64(mut a: f64, mut b: f64) -> f64 {
    a = a.abs();
    b = b.abs();

    if float_eq!(a, 0.0, abs <= EQ_ABS_TOL) {
        return b;
    }

    if float_eq!(b, 0.0, abs <= EQ_ABS_TOL) {
        return a;
    }

    while b >= EQ_ABS_TOL {
        let r = a % b;

        if r.abs() < EQ_ABS_TOL {
            return b;
        }

        a = b;
        b = r.abs();
    }

    a
}
