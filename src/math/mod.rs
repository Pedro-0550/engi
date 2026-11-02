use std::sync::Arc;

use derive_more::*;
use fixed::FixedI128;
use nalgebra::{DMatrix, U86};

use crate::math::complex::Complex;
pub mod complex;
pub mod cordic;
pub mod newton;
pub mod real;

#[derive(Clone, IsVariant, From, Hash)]
pub enum Value {
    Scalar(Complex),
    Matrix(Arc<DMatrix<Complex>>),
}
