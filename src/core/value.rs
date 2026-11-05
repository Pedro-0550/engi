use std::{
    fmt::{Display, Write},
    hash::Hash,
    mem,
    ops::*,
    sync::Arc,
};

use approx::RelativeEq;
use derive_more::IsVariant;
use nalgebra::DMatrix;
// use faer::{Mat, Scale, linalg::solvers::DenseSolveCore};
// use float_eq::{FloatEq, float_eq};
use num::{
    Complex, Float,
    complex::{Complex64, ComplexFloat, c64},
};
use ordered_float::{OrderedFloat, Pow};

// use ordered_float::OrderedFloat;
use crate::{
    core::{
        index::Size,
        util::{ArcExt, impl_as_variant, impl_op_permutations},
    },
    expr::{
        domain::{Domain, Interval},
        shape::Shape,
    },
};

pub const EQ_REL_TOL: f64 = 1e-12;
// pub const ZERO_ABS_TOL: f64 = 1e-24;

// TODO: fixed numbers
/* --------------------------------- TRAITS --------------------------------- */

pub trait ComplexExt
where
    Self: ComplexFloat,
    Self::Real: Float, {
    fn is_integer(&self) -> bool;
    fn is_real(&self) -> bool;
    fn is_imag(&self) -> bool;

    fn as_integer(&self) -> Option<i64>;
    fn as_real(&self) -> Option<f64>;
    fn as_imag(&self) -> Option<f64>;
    fn eq_approx(&self, other: impl Into<Complex64>) -> bool;
}

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, Debug, IsVariant)]
pub enum Value {
    Matrix(Arc<DMatrix<Complex64>>),
    Scalar(Complex64),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(Value, [Matrix => Arc<DMatrix<Complex64>>, Scalar => Complex64]);

// impl<T> From<T> for Value
// where
//     T: Into<Complex64>,
// {
//     fn from(value: T) -> Self {
//         Self::Scalar(value.into())
//     }
// }

impl Default for Value {
    fn default() -> Self {
        Complex64::new(0.0, 0.0).into()
    }
}

impl Value {
    pub fn shape(&self) -> Shape {
        match self {
            Value::Matrix(mat) => {
                Shape::rect(mat.nrows() as Size, mat.ncols() as Size)
            }
            Value::Scalar(_complex) => Shape::SCALAR,
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Scalar(Complex { re: value, im: 0.0 })
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Scalar(Complex { re: value as f64, im: 0.0 })
    }
}

impl From<Complex64> for Value {
    fn from(value: Complex64) -> Self {
        Value::Scalar(value)
    }
}

impl<T: Clone> From<&T> for Value
where
    T: Into<Value>,
{
    fn from(value: &T) -> Self {
        value.clone().into()
    }
}

macro_rules! impl_scalar_func {
    ($name: ident) => {
        pub fn $name(&self) -> Self {
            match self {
                Value::Scalar(complex) => {
                    let result = complex.$name();
                    // assert!(
                    //     !result.is_nan(),
                    //     "Trying to apply {} to {} resulted in NaN",
                    //     stringify!($name),
                    //     self
                    // );
                    result.into()
                }
                _ => todo!(),
            }
        }
    };
}

impl Value {
    pub const ZERO: Value = Value::Scalar(Complex64::ZERO);
    pub const ONE: Value = Value::Scalar(Complex64::ONE);
    pub const I: Value = Value::Scalar(Complex64::I);

    pub fn domain(&self) -> Domain {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(Complex { re, im }) => {
                Domain::new(Interval::singleton(*re), Interval::singleton(*im))
            }
        }
    }

    pub fn realize(&self) -> [Value; 2] {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(complex) => [complex.re.into(), complex.im.into()],
        }
    }

    pub fn precedence(&self) -> u32 {
        match self {
            Value::Matrix(_mat) => 2,
            Value::Scalar(_complex) => 3,
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
            Value::Matrix(mat) => mat.row_iter().all(|r| {
                r.iter().all(|el| {
                    el.re.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
                        && el.im.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
                })
            }),
            Value::Scalar(complex) => {
                complex.re.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
                    && complex.im.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
            }
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

    pub fn as_scalar_real(&self) -> Option<f64> {
        self.as_scalar()?.as_real()
    }

    pub fn as_scalar_imag(&self) -> Option<f64> {
        self.as_scalar()?.as_imag()
    }

    pub fn max(&self, other: &Self) -> Self {
        let a =
            self.as_scalar().expect("max(a, b) is only defined for scalars");
        let b =
            other.as_scalar().expect("max(a, b) is only defined for scalars");

        c64(a.re.max(b.re), a.im.max(b.im)).into()
    }

    pub fn min(&self, other: &Self) -> Self {
        let a =
            self.as_scalar().expect("max(a, b) is only defined for scalars");
        let b =
            other.as_scalar().expect("max(a, b) is only defined for scalars");

        c64(a.re.min(b.re), a.im.min(b.im)).into()
    }

    pub fn atan2(&self, other: &Self) -> Self {
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

        z.conj().into()
    }

    pub fn arg(&self) -> Self {
        let z = self.as_scalar().expect("arg(z) is only defined for scalars");

        z.arg().into()
    }

    pub fn norm(&self) -> Self {
        match self {
            Value::Matrix(mat) => mat.norm().into(),
            Value::Scalar(complex) => complex.norm().into(),
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

impl ComplexExt for Complex64 {
    fn is_integer(&self) -> bool {
        self.im.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
            && self.re.is_finite()
            && self.re.relative_eq(&self.re.round(), f64::EPSILON, EQ_REL_TOL)
    }

    fn is_real(&self) -> bool {
        self.im.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
    }

    fn is_imag(&self) -> bool {
        self.im.relative_ne(&0.0, f64::EPSILON, EQ_REL_TOL)
            && self.re.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL)
    }

    fn as_integer(&self) -> Option<i64> {
        if self.is_integer() { Some(self.re as i64) } else { None }
    }

    fn as_real(&self) -> Option<f64> {
        if self.is_real() { Some(self.re) } else { None }
    }

    fn as_imag(&self) -> Option<f64> {
        if self.is_imag() { Some(self.im) } else { None }
    }

    fn eq_approx(&self, other: impl Into<Complex64>) -> bool {
        let other = other.into();
        self.re.relative_eq(&other.re, f64::EPSILON, EQ_REL_TOL)
            && self.im.relative_eq(&other.im, f64::EPSILON, EQ_REL_TOL)
    }
}

/* -------------------------------------------------------------------------- */

impl Neg for Value {
    type Output = Value;

    fn neg(self) -> Self::Output {
        todo!()
    }
}

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs + rhs)
            }
            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) + (&*rhs)))
            }
            (Value::Matrix(_), Value::Scalar(_))
            | (Value::Scalar(_), Value::Matrix(_)) => {
                panic!("Cannot add a matrix to a scalar, or vice versa")
            }
        }
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs - rhs)
            }
            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*lhs) - (&*rhs)))
            }
            (Value::Matrix(_), Value::Scalar(_))
            | (Value::Scalar(_), Value::Matrix(_)) => {
                panic!("Cannot subtract a matrix from a scalar, or vice versa")
            }
        }
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Value) -> Self::Output {
        match (self, rhs) {
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs * rhs)
            }
            (Value::Scalar(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new((&*rhs) * lhs))
            }
            (Value::Matrix(lhs), Value::Scalar(rhs)) => {
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
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs / rhs)
            }
            (Value::Scalar(lhs), Value::Matrix(rhs)) => Value::Matrix(
                Arc::new((&*rhs).clone().try_inverse().unwrap() * lhs),
            ),
            (Value::Matrix(lhs), Value::Scalar(rhs)) => {
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
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs.pow(rhs))
            }
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
    types = [f64, i64, Complex64, Value],
    exclude_specific = [(Value, Value)],
    exclude_permutations = [f64, i64, Real, Complex64,],
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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Matrix(_), Value::Matrix(_)) => todo!(),
            (Value::Scalar(lhs), Value::Scalar(rhs)) => lhs.eq_approx(*rhs),
            _ => false,
        }
    }
}

impl Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Matrix(mat) => {
                for row in mat.row_iter() {
                    for el in row.iter() {
                        OrderedFloat(el.re).hash(state);
                        OrderedFloat(el.im).hash(state);
                    }
                }
            }
            Value::Scalar(s) => {
                OrderedFloat(s.re).hash(state);
                OrderedFloat(s.im).hash(state);
            }
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Matrix(_mat) => todo!(),
            Value::Scalar(complex) => {
                complex.re.fmt(f)?;

                if complex.re != 0.0 {
                    if complex.im > 0.0 {
                        f.write_str(" + ")?;
                    } else if complex.im < 0.0 {
                        f.write_str(" - ")?;
                    }
                }

                if complex.im != 0.0 {
                    complex.im.fmt(f)?;
                    f.write_char('i')?;
                }

                Ok(())
            }
        }
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

// pub fn mat_exp(_mat: Mat<Complex64>) -> Mat<Complex64> {
//     todo!()
// }

// pub fn mat_ln(_mat: &Mat<Complex64>) -> Mat<Complex64> {
//     todo!()
// }

// pub fn gcd_f64(mut a: f64, mut b: f64) -> f64 {
//     a = a.abs();
//     b = b.abs();

//     if a.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL) {
//         return b;
//     }

//     if a.relative_eq(&0.0, f64::EPSILON, EQ_REL_TOL) {
//         return a;
//     }

//     while b >= EQ_ABS_TOL {
//         let r = a % b;

//         if r.abs() < EQ_ABS_TOL {
//             return b;
//         }

//         a = b;
//         b = r.abs();
//     }

//     a
// }
