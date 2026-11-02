use derive_more::{From, Into};

use crate::math::real::Real;

#[derive(Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct Complex(num::Complex<Real>);
