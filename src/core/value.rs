use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::Hash,
    ops::*,
    sync::Arc,
};

use derive_more::{Deref, DerefMut, From};
use faer::{Mat, MatRef};
use float_eq::float_eq;
use num::{
    Complex, Float, Zero,
    complex::{Complex32, Complex64},
    pow::Pow,
};

use crate::{
    core::util::{impl_as_variant, impl_op_permutations},
    expr::ops::Matrix,
};

pub const EQ_ABS_TOL: f64 = 1e-15;

pub const I: Scalar = Scalar(Complex::I);

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(PartialEq, Clone, Copy, Debug, Deref, DerefMut)]
pub struct Scalar(pub Complex<f64>);

// TODO!
#[derive(Clone, PartialEq, Debug)]
pub struct Set;

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Set(Arc<Set>),
    Matrix(Arc<Mat<Scalar>>),
    Scalar(Scalar),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(Value, [Set => Arc<Set>, Matrix => Arc<Mat<Scalar>>, Scalar => Scalar]);

impl<T> From<T> for Value
where
    T: Into<Scalar>,
{
    fn from(value: T) -> Self {
        Self::Scalar(value.into())
    }
}

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

impl_op_permutations! {
    types = [
        f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, Complex64, Complex32, Scalar
    ],
    exclude_permutations = [f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, Complex64, Complex32],
    exclude_specific = [],
    out = Scalar,

    add = {
        Scalar(lhs.0 + rhs.0)
    },

    mul = {
        Scalar(lhs.0 * rhs.0)
    },

    div = {
        Scalar(lhs.0 / rhs.0)
    },

    sub = {
        Scalar(lhs.0 - rhs.0)
    },

    pow = {
        Scalar(lhs.0.powc(rhs.0))
    },

    partial_eq = {
        lhs == rhs
    }
}

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

impl Div for Value {
    type Output = Value;

    fn div(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl DivAssign for Value {
    fn div_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl AddAssign for Value {
    fn add_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl SubAssign for Value {
    fn sub_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Self) -> Self::Output {
        todo!()
    }
}

impl MulAssign for Value {
    fn mul_assign(&mut self, rhs: Self) {
        todo!()
    }
}

impl_op_permutations! {
    types = [
        f64, i64, Complex64, Complex32, Scalar, Set, &Set, Value, &Value
    ],
    exclude_permutations = [f64, i64, Complex64, Complex32, Set, &Set],
    exclude_specific = [(Value, Value)],
    out = Value,

    add = {
        todo!()
    },

    mul = {
        todo!()
    },

    div = {
        todo!()
    },

    sub = {
        todo!()
    },

    pow = {
        todo!()
    },

    partial_eq = {
        todo!()
    }
}

impl Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        todo!()
    }
}
