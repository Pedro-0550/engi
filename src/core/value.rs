use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::Hash,
    ops::*,
    sync::Arc,
};

use derive_more::{Deref, DerefMut, From};
use faer::{
    Mat, MatRef, Scale, Side, linalg::solvers::DenseSolveCore,
    traits::ComplexField,
};
use float_eq::float_eq;
use num::{
    Complex, Float, Zero,
    complex::{Complex32, Complex64, ComplexFloat},
    pow::Pow,
};
use ordered_float::OrderedFloat;

use crate::{
    core::util::{ArcExt, impl_as_variant, impl_op_permutations},
    expr::{Shape, Shaped, ops::Matrix},
};

pub const EQ_ABS_TOL: f64 = 1e-18;

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
}

/* --------------------------------- STRUCTS -------------------------------- */

// TODO!
#[derive(Clone, PartialEq, Debug)]
pub struct Set;

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Set(Arc<Set>),
    Matrix(Arc<Mat<Complex64>>),
    Scalar(Complex64),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(Value, [Set => Arc<Set>, Matrix => Arc<Mat<Complex64>>, Scalar => Complex64]);

// impl<T> From<T> for Value
// where
//     T: Into<Complex64>,
// {
//     fn from(value: T) -> Self {
//         Self::Scalar(value.into())
//     }
// }

impl Shaped for Value {
    fn shape(&self) -> crate::expr::Shape {
        match self {
            Value::Set(set) => todo!(),
            Value::Matrix(mat) => Shape::rect(mat.nrows(), mat.ncols()),
            Value::Scalar(complex) => Shape::SCALAR,
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

impl From<Set> for Value {
    fn from(value: Set) -> Self {
        Value::Set(Arc::new(value))
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

impl Value {
    pub const ZERO: Value = Value::Scalar(Complex64::ZERO);
    pub const ONE: Value = Value::Scalar(Complex64::ONE);
    pub const I: Value = Value::Scalar(Complex64::I);

    pub fn precedence(&self) -> u32 {
        match self {
            Value::Set(set) => 1,
            Value::Matrix(mat) => 2,
            Value::Scalar(complex) => 3,
        }
    }

    /// Returns the n by n identity matrix, or 1 for n = 1
    fn identity(n: usize) -> Value {
        if n == 1 {
            1.0.into()
        } else {
            Value::Matrix(Arc::new(Mat::identity(n, n)))
        }
    }

    /// Returns the n by n zero matrix, or 0 for n = 1
    fn zero(n: usize) -> Value {
        if n == 0 {
            0.0.into()
        } else {
            Value::Matrix(Arc::new(Mat::zeros(n, n)))
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

    pub fn norm(&self) -> Self {
        todo!();
    }

    pub fn sin(&self) -> Self {
        todo!();
    }

    pub fn cos(&self) -> Self {
        todo!();
    }

    pub fn tan(&self) -> Self {
        todo!();
    }

    pub fn asin(&self) -> Self {
        todo!();
    }

    pub fn acos(&self) -> Self {
        todo!();
    }

    pub fn atan(&self) -> Self {
        todo!();
    }

    pub fn sinh(&self) -> Self {
        todo!();
    }

    pub fn cosh(&self) -> Self {
        todo!();
    }

    pub fn tanh(&self) -> Self {
        todo!();
    }

    pub fn asinh(&self) -> Self {
        todo!();
    }

    pub fn acosh(&self) -> Self {
        todo!();
    }

    pub fn atanh(&self) -> Self {
        todo!();
    }
}

impl ComplexExt for Complex64 {
    fn is_integer(&self) -> bool {
        self.im.abs() > EQ_ABS_TOL
            && self.re.is_finite()
            && float_eq!(self.re, self.re.round(), abs <= EQ_ABS_TOL)
    }

    fn is_real(&self) -> bool {
        float_eq!(self.im, 0.0, abs <= EQ_ABS_TOL)
    }

    fn is_imag(&self) -> bool {
        self.im.abs() > EQ_ABS_TOL && self.re.abs() <= EQ_ABS_TOL
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
}

impl Neg for Value {
    type Output = Value;

    fn neg(self) -> Self::Output {
        todo!()
    }
}

impl_op_permutations! {
    types = [
        f64, i64, Complex64, Set, &Set, Value, &Value
    ],
    exclude_permutations = [f64, i64, Complex64, Set, &Set],
    out = Value,

    add = {
        match (lhs, rhs) {
            (Value::Set(_), Value::Set(_)) => todo!(),
            (Value::Set(set), Value::Matrix(mat)) => todo!(),
            (Value::Set(set), Value::Scalar(complex)) => todo!(),
            (Value::Matrix(mat), Value::Set(set)) => todo!(),
            (Value::Scalar(complex), Value::Set(set)) => todo!(),

            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new(lhs.make_owned() + rhs.make_owned()))
            }
            (Value::Matrix(mat), Value::Scalar(scalar))
            | (Value::Scalar(scalar), Value::Matrix(mat)) => {
                panic!("Cannot add a scalar to a matrix")
            }
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs + rhs)
            }
        }
    },

    mul = {
        match (lhs, rhs) {
            (Value::Set(_), Value::Set(_)) => todo!(),
            (Value::Set(set), Value::Matrix(mat)) => todo!(),
            (Value::Set(set), Value::Scalar(complex)) => todo!(),
            (Value::Matrix(mat), Value::Set(set)) => todo!(),
            (Value::Scalar(complex), Value::Set(set)) => todo!(),

            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new(lhs.make_owned() * rhs.make_owned()))
            }
            (Value::Matrix(mat), Value::Scalar(scalar))
            | (Value::Scalar(scalar), Value::Matrix(mat)) => {
                Value::Matrix(Arc::new(mat.make_owned() * Scale(scalar)))
            }
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs * rhs)
            }
        }
    },

    div = {
        match (lhs, rhs) {
            (Value::Set(_), Value::Set(_)) => todo!(),
            (Value::Set(set), Value::Matrix(mat)) => todo!(),
            (Value::Set(set), Value::Scalar(complex)) => todo!(),
            (Value::Matrix(mat), Value::Set(set)) => todo!(),
            (Value::Scalar(complex), Value::Set(set)) => todo!(),

            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new(lhs.make_owned() * rhs.make_owned().partial_piv_lu().inverse()))
            }
            (Value::Matrix(mat), Value::Scalar(scalar))
            | (Value::Scalar(scalar), Value::Matrix(mat)) => {
                Value::Matrix(Arc::new(mat.make_owned() / Scale(scalar)))
            }
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs / rhs)
            }
        }
    },

    sub = {
        match (lhs, rhs) {
            (Value::Set(_), Value::Set(_)) => todo!(),
            (Value::Set(set), Value::Matrix(mat)) => todo!(),
            (Value::Set(set), Value::Scalar(complex)) => todo!(),
            (Value::Matrix(mat), Value::Set(set)) => todo!(),
            (Value::Scalar(complex), Value::Set(set)) => todo!(),

            (Value::Matrix(lhs), Value::Matrix(rhs)) => {
                Value::Matrix(Arc::new(lhs.make_owned() - rhs.make_owned()))
            }
            (Value::Matrix(mat), Value::Scalar(scalar))
            | (Value::Scalar(scalar), Value::Matrix(mat)) => {
                panic!("Cannot subtract a scalar from a matrix, or vice versa")
            }
            (Value::Scalar(lhs), Value::Scalar(rhs)) => {
                Value::Scalar(lhs - rhs)
            }
        }
    },

    pow = {
        match (lhs, rhs) {
            (Value::Set(_), Value::Set(_)) => todo!(),
            (Value::Set(set), Value::Matrix(mat)) => todo!(),
            (Value::Set(set), Value::Scalar(complex)) => todo!(),
            (Value::Matrix(mat), Value::Set(set)) => todo!(),
            (Value::Scalar(complex), Value::Set(set)) => todo!(),

            (Value::Matrix(base), Value::Matrix(exp)) => {
                // A^B = exp(B * log(A))
                Value::Matrix(Arc::new(mat_exp(exp.make_owned() * mat_ln(&base))))
            }
            (Value::Matrix(base), Value::Scalar(exp)) => {
                // A^n = exp(log(A) * n)
                Value::Matrix(Arc::new(mat_exp(mat_ln(&base) * Scale(exp))))
            }
            (Value::Scalar(base), Value::Matrix(exp)) => {
                // n^A = exp(log(n) * A)
                Value::Matrix(Arc::new(mat_exp(Scale(base.ln()) * exp.make_owned())))
            }
            (Value::Scalar(base), Value::Scalar(exp)) => {
                Value::Scalar(base.powc(exp))
            }
        }
    },

    partial_eq = {
        lhs == rhs
    }
}

impl Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Set(set) => todo!(),
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
            Value::Set(set) => todo!(),
            Value::Matrix(mat) => todo!(),
            Value::Scalar(complex) => {
                complex.re.fmt(f)?;

                if complex.re != 0.0 && complex.im != 0.0 {
                    f.write_str(" + ")?;
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

pub fn mat_exp(mat: Mat<Complex64>) -> Mat<Complex64> {
    todo!()
}

pub fn mat_ln(mat: &Mat<Complex64>) -> Mat<Complex64> {
    todo!()
}

pub fn gcd_f64(mut a: f64, mut b: f64) -> f64 {
    a = a.abs();
    b = b.abs();

    if a <= EQ_ABS_TOL {
        return b;
    }

    if b <= EQ_ABS_TOL {
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
