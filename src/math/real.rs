use std::{
    fmt::Display,
    ops::{Neg, Shl, Shr},
};

use approx::{AbsDiffEq, RelativeEq, UlpsEq};
use derive_more::*;
use fixed::{FixedI128, traits::Fixed, types::extra::U86};
use nalgebra::{ComplexField, Field, RealField, SimdValue};
use num::{FromPrimitive, Num, One, Signed, Zero};
use simba::{scalar::SubsetOf, simd::PrimitiveSimdValue};

type Base = FixedI128<U86>;

#[derive(
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Copy,
    Debug,
    Default,
    From,
    Into,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    RemAssign,
)]
#[mul(forward)]
#[mul_assign(forward)]
#[div(forward)]
#[div_assign(forward)]
#[rem(forward)]
#[rem_assign(forward)]
pub struct Real(Base);

impl Neg for Real {
    type Output = Real;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Shr<u8> for Real {
    type Output = Real;

    fn shr(self, s: u8) -> Self::Output {
        Self(self.0 >> s)
    }
}

impl Shl<u8> for Real {
    type Output = Real;

    fn shl(self, s: u8) -> Self::Output {
        Self(self.0 << s)
    }
}

impl cordic::CordicNumber for Real {
    fn floor(self) -> Self {
        Self(self.0.floor())
    }

    fn zero() -> Self {
        Self(Base::ZERO)
    }

    fn one() -> Self {
        Self(Base::ONE)
    }

    fn frac_pi_2() -> Self {
        Self(Base::FRAC_2_PI)
    }

    fn pi() -> Self {
        Self(Base::PI)
    }

    fn e() -> Self {
        Self(Base::E)
    }

    fn from_u0f64(val: fixed::types::U0F64) -> Self {
        Real(val.to_num())
    }

    fn num_fract_bits() -> u8 {
        86
    }

    fn num_bits() -> u8 {
        127
    }
}

// impl SimdValue for Real {

// }

// impl Field for Real {}

impl Num for Real {
    type FromStrRadixErr = ();

    fn from_str_radix(
        str: &str,
        radix: u32,
    ) -> Result<Self, Self::FromStrRadixErr> {
        todo!()
    }
}

impl Zero for Real {
    fn zero() -> Self {
        Real(0.into())
    }

    fn is_zero(&self) -> bool {
        self == 0
    }
}

impl One for Real {
    fn one() -> Self {
        Real(1.into())
    }
}

impl Signed for Real {
    fn abs(&self) -> Self {
        Real(self.0.abs())
    }

    fn abs_sub(&self, other: &Self) -> Self {
        Real(self.0.abs_diff(other.0).to_num())
    }

    fn signum(&self) -> Self {
        Real(self.0.signum())
    }

    fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    fn is_negative(&self) -> bool {
        self.0.is_negative()
    }
}

impl FromPrimitive for Real {
    fn from_i64(n: i64) -> Option<Self> {
        Base::checked_from_num(n).map(Real)
    }

    fn from_u64(n: u64) -> Option<Self> {
        Base::checked_from_num(n).map(Real)
    }
}

impl SubsetOf<Real> for f64 {
    fn to_superset(&self) -> Real {
        Real(Base::from_num(self))
    }

    fn from_superset_unchecked(element: &Real) -> Self {
        element.0.to_num()
    }

    fn is_in_subset(element: &Real) -> bool {
        true
    }
}

impl SubsetOf<Real> for f32 {
    fn to_superset(&self) -> Real {
        Real(Base::from_num(self))
    }

    fn from_superset_unchecked(element: &Real) -> Self {
        element.0.to_num()
    }

    fn is_in_subset(element: &Real) -> bool {
        true
    }
}

impl SubsetOf<Real> for Real {
    fn to_superset(&self) -> Real {
        *self
    }

    fn from_superset_unchecked(element: &Real) -> Self {
        *element
    }

    fn is_in_subset(element: &Real) -> bool {
        true
    }
}

impl AbsDiffEq for Real {
    type Epsilon = Self;

    fn default_epsilon() -> Self::Epsilon {
        Self(Base::DELTA)
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.abs_sub(other) < epsilon
    }
}

impl RelativeEq for Real {
    fn default_max_relative() -> Self::Epsilon {
        Self(Base::DELTA)
    }

    fn relative_eq(
        &self,
        other: &Self,
        epsilon: Self::Epsilon,
        max_relative: Self::Epsilon,
    ) -> bool {
        todo!()
    }
}

impl UlpsEq for Real {
    fn default_max_ulps() -> u32 {
        todo!()
    }

    fn ulps_eq(
        &self,
        other: &Self,
        epsilon: Self::Epsilon,
        max_ulps: u32,
    ) -> bool {
        todo!()
    }
}

macro_rules! impl_primitive_simd_value_for_scalar (
    ($($t: ty),*) => {$(
        impl PrimitiveSimdValue for $t {}
        impl SimdValue for $t {
            const LANES: usize = 1;
            type Element = $t;
            type SimdBool = bool;

            #[inline(always)]
            fn splat(val: Self::Element) -> Self {
                val
            }

            #[inline(always)]
            fn extract(&self, _: usize) -> Self::Element {
                *self
            }

            #[inline(always)]
            unsafe fn extract_unchecked(&self, _: usize) -> Self::Element {
                *self
            }

            #[inline(always)]
            fn replace(&mut self, _: usize, val: Self::Element) {
                *self = val
            }

            #[inline(always)]
            unsafe fn replace_unchecked(&mut self, _: usize, val: Self::Element) {
                *self = val
            }

            #[inline(always)]
            fn select(self, cond: Self::SimdBool, other: Self) -> Self {
                if cond {
                    self
                } else {
                    other
                }
            }
        }
    )*}
);

impl_primitive_simd_value_for_scalar!(Real);

impl Field for Real {}

impl RealField for Real {
    fn is_sign_positive(&self) -> bool {
        self.0.is_positive()
    }

    fn is_sign_negative(&self) -> bool {
        self.0.is_negative()
    }

    fn copysign(self, sign: Self) -> Self {
        Real(self.0.abs() * sign.0.signum())
    }

    fn max(self, other: Self) -> Self {
        self.max(other)
    }

    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    fn clamp(self, min: Self, max: Self) -> Self {
        self.clamp(min, max)
    }

    fn atan2(self, other: Self) -> Self {
        cordic::atan2(self, other)
    }

    fn min_value() -> Option<Self> {
        Some(Self(Base::DELTA))
    }

    fn max_value() -> Option<Self> {
        Some(Self(Base::MAX))
    }

    fn pi() -> Self {
        Self(Base::PI)
    }

    fn two_pi() -> Self {
        Self(Base::TAU)
    }

    fn frac_pi_2() -> Self {
        Self(Base::FRAC_PI_2)
    }

    fn frac_pi_3() -> Self {
        Self(Base::FRAC_PI_3)
    }

    fn frac_pi_4() -> Self {
        Self(Base::FRAC_PI_4)
    }

    fn frac_pi_6() -> Self {
        Self(Base::FRAC_PI_6)
    }

    fn frac_pi_8() -> Self {
        Self(Base::FRAC_PI_8)
    }

    fn frac_1_pi() -> Self {
        Self(Base::FRAC_1_PI)
    }

    fn frac_2_pi() -> Self {
        Self(Base::FRAC_2_PI)
    }

    fn frac_2_sqrt_pi() -> Self {
        Self(Base::FRAC_2_SQRT_PI)
    }

    fn e() -> Self {
        Self(Base::E)
    }

    fn log2_e() -> Self {
        Self(Base::LOG2_E)
    }

    fn log10_e() -> Self {
        Self(Base::LOG10_E)
    }

    fn ln_2() -> Self {
        Self(Base::LN_2)
    }

    fn ln_10() -> Self {
        Self(Base::LN_10)
    }
}

impl ComplexField for Real {
    type RealField = Real;

    #[inline]
    fn from_real(re: Self::RealField) -> Self {
        re
    }

    #[inline]
    fn real(self) -> Self::RealField {
        self
    }

    #[inline]
    fn imaginary(self) -> Self::RealField {
        Self::zero()
    }

    #[inline]
    fn norm1(self) -> Self::RealField {
        self.abs()
    }

    #[inline]
    fn modulus(self) -> Self::RealField {
        self.abs()
    }

    #[inline]
    fn modulus_squared(self) -> Self::RealField {
        self * self
    }

    #[inline]
    fn argument(self) -> Self::RealField {
        if self >= Self::zero() { Self::zero() } else { Self::pi() }
    }

    #[inline]
    fn to_exp(self) -> (Self, Self) {
        if self >= Self::zero() {
            (self, Self::one())
        } else {
            (-self, -Self::one())
        }
    }

    #[inline]
    fn recip(self) -> Self {
        Self(self.0.recip())
    }

    #[inline]
    fn conjugate(self) -> Self {
        self
    }

    #[inline]
    fn scale(self, factor: Self::RealField) -> Self {
        self * factor
    }

    #[inline]
    fn unscale(self, factor: Self::RealField) -> Self {
        self / factor
    }

    fn sin(self) -> Self {
        self.sin_cos().0
    }

    fn cos(self) -> Self {
        self.sin_cos().1
    }

    fn sin_cos(self) -> (Self, Self) {
        cordic::sin_cos(self)
    }

    fn tan(self) -> Self {
        cordic::tan(self)
    }

    fn asin(self) -> Self {
        cordic::asin(self)
    }

    fn acos(self) -> Self {
        cordic::acos(self)
    }

    fn atan(self) -> Self {
        cordic::atan(self)
    }

    fn sinh(self) -> Self {
        todo!()
    }

    fn cosh(self) -> Self {
        todo!()
    }

    fn tanh(self) -> Self {
        todo!()
    }

    fn asinh(self) -> Self {
        todo!()
    }

    fn acosh(self) -> Self {
        todo!()
    }

    fn atanh(self) -> Self {
        todo!()
    }

    fn log(self, base: Self::RealField) -> Self {
        todo!()
    }

    fn log2(self) -> Self {
        todo!()
    }

    fn log10(self) -> Self {
        todo!()
    }

    fn ln(self) -> Self {
        todo!()
    }

    fn ln_1p(self) -> Self {
        todo!()
    }

    fn sqrt(self) -> Self {
        cordic::sqrt(self)
    }

    fn exp(self) -> Self {
        cordic::exp(self)
    }

    fn exp2(self) -> Self {
        todo!()
    }

    fn exp_m1(self) -> Self {
        todo!()
    }

    fn powi(self, n: i32) -> Self {
        todo!()
    }

    fn powf(self, n: Self::RealField) -> Self {
        todo!()
    }

    fn powc(self, n: Self) -> Self {
        todo!()
    }

    fn cbrt(self) -> Self {
        todo!()
    }

    fn is_finite(&self) -> bool {
        true
    }

    fn try_sqrt(self) -> Option<Self> {
        Some(self.sqrt())
    }
}

impl Display for Real {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Base as Display>::fmt(&self.0, f)
    }
}
