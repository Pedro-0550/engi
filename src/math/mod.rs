use std::sync::Arc;

use derive_more::*;
use fixed::FixedI128;
use nalgebra::{DMatrix, U86};

use crate::{
    c,
    math::{Value::Scalar, complex::Complex},
};
pub mod complex;
mod cordic;
mod newton;
pub mod real;

#[derive(Clone, IsVariant, From, Hash)]
pub enum Value {
    Scalar(Complex),
    Matrix(Arc<DMatrix<Complex>>),
}

impl Default for Value {
    fn default() -> Self {
        Scalar(c!(0))
    }
}
