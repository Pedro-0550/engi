use std::{
    fmt::{
        Binary, Debug, Display, LowerExp, LowerHex, Octal, UpperExp, UpperHex,
    },
    mem,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
    sync::Arc,
};

use derive_more::*;
use fixed::FixedI128;
use nalgebra::{ComplexField, DMatrix, U86};
use num::{Zero, pow::Pow};

use crate::{
    c,
    core::util::{impl_as_variant, impl_op_permutations},
    math::{Value::Scalar, complex::Complex, real::Real},
};
pub mod complex;
mod cordic;
mod newton;
pub mod real;

#[derive(Clone, IsVariant, From, Hash, PartialEq, Eq)]
pub enum Value {
    Scalar(Complex),
    Matrix(Arc<DMatrix<Complex>>),
}

impl Default for Value {
    fn default() -> Self {
        Scalar(c!(0))
    }
}

impl_as_variant!(Value, [Matrix => Arc<DMatrix<Complex>>, Scalar => Complex]);

impl<T> From<T> for Value
where
    Complex: From<T>,
{
    #[inline]
    default fn from(value: T) -> Self {
        Value::Scalar(value.into())
    }
}

/* -------------------------------------------------------------------------- */

macro_rules! impl_scalar_func {
    ($name: ident) => {
        pub fn $name(&self) -> Self {
            match self {
                Value::Scalar(complex) => {
                    let result = complex.$name();
                    Value::Scalar(result)
                }
                _ => todo!(),
            }
        }
    };
}

impl Value {
    pub const ZERO: Value = Value::Scalar(Complex::ZERO);
    pub const ONE: Value = Value::Scalar(Complex::ONE);
    pub const I: Value = Value::Scalar(Complex::I);

    // pub fn domain(&self) -> Domain {
    //     match self {
    //         Value::Set(_set) => todo!(),
    //         Value::Matrix(_mat) => todo!(),
    //         Value::Scalar(Complex { re, im }) => {
    //             Domain::new(Interval::singleton(*re), Interval::singleton(*im))
    //         }
    //     }
    // }

    pub fn realize(&self) -> [Value; 2] {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(complex) => [complex.re.into(), complex.im.into()],
        }
    }

    pub fn precedence(&self) -> u32 {
        match self {
            Value::Matrix(_mat) => 0,
            Value::Scalar(_complex) => 1,
        }
    }

    /// Returns the n by n identity matrix, or 1 for n = 1
    fn identity(n: usize) -> Value {
        if n == 1 {
            1.0.into()
        } else {
            Value::Matrix(Arc::new(DMatrix::identity(n, n)))
        }
    }

    /// Returns the n by n zero matrix, or 0 for n = 1
    fn zero(n: usize) -> Value {
        if n == 1 {
            0.0.into()
        } else {
            Value::Matrix(Arc::new(DMatrix::zeros(n, n)))
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Value::Matrix(mat) => {
                mat.row_iter().all(|r| r.iter().all(|x| x.is_zero()))
            }
            Value::Scalar(complex) => complex.is_zero(),
        }
    }

    pub fn is_scalar_integer(&self) -> bool {
        self.as_scalar().is_some_and(|s| s.is_integer())
    }

    pub fn is_scalar_real(&self) -> bool {
        self.as_scalar().is_some_and(|s| s.is_real())
    }

    pub fn is_scalar_imag(&self) -> bool {
        self.as_scalar().is_some_and(|s| s.is_imag())
    }

    pub fn as_scalar_integer(&self) -> Option<i64> {
        self.as_scalar()?.as_integer()
    }

    pub fn as_scalar_real(&self) -> Option<Real> {
        self.as_scalar()?.as_real()
    }

    pub fn as_scalar_imag(&self) -> Option<Real> {
        self.as_scalar()?.as_imag()
    }

    pub fn max(&self, other: &Self) -> Self {
        let a =
            self.as_scalar().expect("max(a, b) is only defined for scalars");
        let b =
            other.as_scalar().expect("max(a, b) is only defined for scalars");

        Complex::new(a.re.max(b.re), a.im.max(b.im)).into()
    }

    pub fn min(&self, other: &Self) -> Self {
        let a =
            self.as_scalar().expect("max(a, b) is only defined for scalars");
        let b =
            other.as_scalar().expect("max(a, b) is only defined for scalars");

        Complex::new(a.re.min(b.re), a.im.min(b.im)).into()
    }

    pub fn atan2(&self, other: &Self) -> Self {
        use nalgebra::RealField;
        let x = self
            .as_scalar_real()
            .expect("atan2(x, y) is only defined for real scalars");
        let y = other
            .as_scalar_real()
            .expect("atan2(x, y) is only defined for real scalars");

        y.atan2(x).into()
    }

    pub fn conj(&self) -> Self {
        let z = self.as_scalar().expect("conj(z) is only defined for scalars");

        z.conjugate().into()
    }

    pub fn arg(&self) -> Self {
        let z = self.as_scalar().expect("arg(z) is only defined for scalars");

        z.argument().into()
    }

    pub fn norm(&self) -> Self {
        match self {
            Value::Matrix(mat) => mat.norm().into(),
            Value::Scalar(complex) => complex.modulus().into(),
        }
    }

    pub fn ln(&self) -> Self {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(complex) => complex.ln().into(),
        }
    }

    pub fn sign(&self) -> Self {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(c) => {
                Complex::new(c.re.signum(), c.im.signum()).into()
            }
        }
    }

    impl_scalar_func!(cos);
    impl_scalar_func!(sin);
    impl_scalar_func!(tan);
    impl_scalar_func!(acos);
    impl_scalar_func!(asin);
    impl_scalar_func!(atan);
    impl_scalar_func!(cosh);
    impl_scalar_func!(sinh);
    impl_scalar_func!(tanh);
    impl_scalar_func!(acosh);
    impl_scalar_func!(asinh);
    impl_scalar_func!(atanh);
}

/* -------------------------------------------------------------------------- */

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Scalar(lhs), Scalar(rhs)) => Scalar(lhs + rhs),
            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) + (&*rhs)))
            }
            (Value::Matrix(_), Scalar(_)) | (Scalar(_), Value::Matrix(_)) => {
                panic!("Cannot add a matrix to a scalar, or vice versa")
            }
        }
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Scalar(lhs), Scalar(rhs)) => Scalar(lhs - rhs),
            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) - (&*rhs)))
            }
            (Value::Matrix(_), Scalar(_)) | (Scalar(_), Value::Matrix(_)) => {
                panic!("Cannot subtract a matrix from a scalar, or vice versa")
            }
        }
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Scalar(lhs), Scalar(rhs)) => Scalar(lhs * rhs),
            (Scalar(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*rhs) * lhs))
            }
            (Value::Matrix(lhs), Scalar(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) * rhs))
            }
            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) * (&*rhs)))
            }
        }
    }
}

impl Div for Value {
    type Output = Value;

    fn div(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Scalar(lhs), Scalar(rhs)) => Scalar(lhs / rhs),
            (Scalar(lhs), Value::Matrix(rhs)) => Value::Matrix(Arc::new(
                (&*rhs).clone().try_inverse().unwrap() * lhs,
            )),
            (Value::Matrix(lhs), Scalar(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) / rhs))
            }
            (Value::Matrix(lhs), Value::Matrix(rhs)) => Value::Matrix(
                Arc::new((&*lhs) * (&*rhs).clone().try_inverse().unwrap()),
            ),
        }
    }
}

impl Pow<Self> for Value {
    type Output = Value;

    fn pow(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Scalar(lhs), Scalar(rhs)) => Scalar(lhs.pow(rhs)),
            _ => todo!(),
        }
    }
}

impl AddAssign for Value {
    fn add_assign(&mut self, rhs: Self) {
        *self = mem::take(self) + rhs
    }
}

impl MulAssign for Value {
    fn mul_assign(&mut self, rhs: Self) {
        *self = mem::take(self) * rhs
    }
}

impl SubAssign for Value {
    fn sub_assign(&mut self, rhs: Self) {
        *self = mem::take(self) - rhs
    }
}

impl DivAssign for Value {
    fn div_assign(&mut self, rhs: Self) {
        *self = mem::take(self) / rhs
    }
}

impl_op_permutations!(
    types = [f64, i64, Real, Complex, Value],
    exclude_specific = [(Value, Value)],
    exclude_permutations = [f64, i64, Real, Complex,],
    into = Value,
    out = Value,
    add = { lhs + rhs },
    sub = { lhs - rhs },
    mul = { lhs * rhs },
    div = { lhs / rhs },
    pow = { lhs.pow(rhs) },
    partial_eq = { lhs == rhs },
);

/* -------------------------------------------------------------------------- */

macro_rules! impl_value_fmt {
    ($trait:ident) => {
        impl $trait for Value {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Value::Scalar(x) => $trait::fmt(x, f),
                    Value::Matrix(mat) => $trait::fmt(&**mat, f),
                }
            }
        }
    };
}

impl_value_fmt!(Display);
impl_value_fmt!(Debug);
impl_value_fmt!(Octal);
impl_value_fmt!(Binary);
impl_value_fmt!(LowerExp);
impl_value_fmt!(UpperExp);
impl_value_fmt!(LowerHex);
impl_value_fmt!(UpperHex);
