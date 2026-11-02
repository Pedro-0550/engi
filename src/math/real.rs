use std::{
    fmt::{
        Binary, Debug, Display, Formatter, LowerExp, LowerHex, Octal, UpperExp,
        UpperHex,
    },
    ops::{Neg, Shl, ShlAssign, Shr, ShrAssign},
};

use approx::{AbsDiffEq, RelativeEq, UlpsEq};
use derive_more::*;
use fixed::{
    FixedI128, ParseFixedError,
    traits::{Fixed, FromFixed, ToFixed},
    types::extra::U86,
};
use nalgebra::{ComplexField, Field, RealField, SimdValue};
use num::{FromPrimitive, Num, One, Signed, Zero, pow::Pow};
use simba::{scalar::SubsetOf, simd::PrimitiveSimdValue};

use crate::{
    core::util::impl_op_permutations,
    math::{
        cordic::{
            acos, acosh, asin, asinh, atan, atanh, exp, ln, sin_cos, sinh_cosh,
        },
        newton::{cbrt, sqrt},
    },
};

pub type Base = FixedI128<U86>;

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

#[macro_export]
macro_rules! r {
    ($lit:literal) => {
        $crate::math::real::Real::from($crate::math::real::Base::lit(
            stringify!($lit),
        ))
    };
}

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

impl ShrAssign<u8> for Real {
    fn shr_assign(&mut self, rhs: u8) {
        *self = *self >> rhs
    }
}

impl Shl<u8> for Real {
    type Output = Real;

    fn shl(self, s: u8) -> Self::Output {
        Self(self.0 << s)
    }
}

impl ShlAssign<u8> for Real {
    fn shl_assign(&mut self, rhs: u8) {
        *self = *self << rhs
    }
}

impl Num for Real {
    type FromStrRadixErr = ParseFixedError;

    fn from_str_radix(
        str: &str,
        radix: u32,
    ) -> Result<Self, Self::FromStrRadixErr> {
        match radix {
            2 => Base::from_str_binary(str).map(Real),
            8 => Base::from_str_octal(str).map(Real),
            10 => Base::from_str(str).map(Real),
            16 => Base::from_str_hex(str).map(Real),
            _ => unimplemented!(),
        }
    }
}

impl Zero for Real {
    fn zero() -> Self {
        Real(0.into())
    }

    fn is_zero(&self) -> bool {
        *self == 0
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
        Real(Base::from_num(*self))
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
        Real(Base::from_num(*self))
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
        let abs_diff = self.abs_sub(other);

        if abs_diff <= epsilon {
            return true;
        }

        let abs_self = self.abs();
        let abs_other = other.abs();

        let largest = if abs_other > abs_self { abs_other } else { abs_self };

        abs_diff <= largest * max_relative
    }
}

impl UlpsEq for Real {
    fn default_max_ulps() -> u32 {
        unimplemented!()
    }

    fn ulps_eq(
        &self,
        other: &Self,
        epsilon: Self::Epsilon,
        max_ulps: u32,
    ) -> bool {
        unimplemented!()
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
        Ord::max(self, other)
    }

    fn min(self, other: Self) -> Self {
        Ord::min(self, other)
    }

    fn clamp(self, min: Self, max: Self) -> Self {
        Ord::clamp(self, min, max)
    }

    fn atan2(self, other: Self) -> Self {
        let x = self;
        let y = other;

        if x > r!(0) {
            atan(y / x)
        } else if x < Real::ZERO && y >= Real::ZERO {
            atan(y / x) + Real::PI
        } else if x < Real::ZERO {
            atan(y / x) - Real::PI
        } else if y > Real::ZERO {
            Real::FRAC_PI_2
        } else if y < Real::ZERO {
            -Real::FRAC_PI_2
        } else {
            Real::ZERO // x == 0 && y == 0, undefined; 0 by convention
        }
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
        if self >= Self::ZERO { Self::ZERO } else { Self::PI }
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
        sin_cos(self)
    }

    fn tan(self) -> Self {
        let (sin, cos) = self.sin_cos();

        sin / cos
    }

    fn asin(self) -> Self {
        asin(self)
    }

    fn acos(self) -> Self {
        acos(self)
    }

    fn atan(self) -> Self {
        atan(self)
    }

    fn sinh_cosh(self) -> (Self, Self) {
        sinh_cosh(self)
    }

    fn sinh(self) -> Self {
        self.sinh_cosh().0
    }

    fn cosh(self) -> Self {
        self.sinh_cosh().1
    }

    fn tanh(self) -> Self {
        let (sinh, cosh) = self.sinh_cosh();
        sinh / cosh
    }

    fn asinh(self) -> Self {
        asinh(self)
    }

    fn acosh(self) -> Self {
        acosh(self)
    }

    fn atanh(self) -> Self {
        atanh(self)
    }

    fn log(self, base: Self::RealField) -> Self {
        ln(self) / ln(base)
    }

    fn log2(self) -> Self {
        ln(self) / Self::ln_2()
    }

    fn log10(self) -> Self {
        ln(self) / Self::ln_10()
    }

    fn ln(self) -> Self {
        ln(self)
    }

    fn ln_1p(self) -> Self {
        ln(r!(1) + self)
    }

    fn sqrt(self) -> Self {
        sqrt(self)
    }

    fn exp(self) -> Self {
        exp(self)
    }

    fn exp2(self) -> Self {
        r!(2).powf(self)
    }

    fn exp_m1(self) -> Self {
        exp(self) - r!(1)
    }

    fn powi(self, n: i32) -> Self {
        exp(Real::from(n) * ln(self))
    }

    fn powf(self, n: Self::RealField) -> Self {
        self.pow(n)
    }

    fn powc(self, n: Self) -> Self {
        self.pow(n)
    }

    fn cbrt(self) -> Self {
        cbrt(self)
    }

    fn is_finite(&self) -> bool {
        true
    }

    fn try_sqrt(self) -> Option<Self> {
        Some(self.sqrt())
    }

    fn floor(self) -> Self {
        Self(self.0.floor())
    }

    fn ceil(self) -> Self {
        Self(self.0.ceil())
    }

    fn round(self) -> Self {
        Self(self.0.round())
    }

    fn trunc(self) -> Self {
        Self(self.0.int())
    }

    fn fract(self) -> Self {
        Self(self.0.frac() * self.0.signum())
    }

    fn mul_add(self, a: Self, b: Self) -> Self {
        Self(self.0.mul_add(a.0, b.0))
    }

    fn abs(self) -> Self::RealField {
        Self(self.0.abs())
    }

    fn hypot(self, other: Self) -> Self::RealField {
        sqrt(self * self + other * other)
    }
}

impl Real {
    pub const ZERO: Real = r!(0);
    pub const ONE: Real = r!(1);

    pub const MAX: Real = Real(Base::MAX);
    pub const MIN: Real = Real(Base::MIN);
    pub const DELTA: Real = Real(Base::DELTA);
    pub const EPSILON: Real = Real(Base::DELTA);

    pub const FRAC_NBITS: u8 = Base::FRAC_NBITS as u8;
    pub const INT_NBITS: u8 = Base::INT_NBITS as u8;

    pub const PI: Real = Real(Base::PI);
    pub const TAU: Real = Real(Base::TAU);
    pub const FRAC_PI_2: Real = Real(Base::FRAC_PI_2);
    pub const FRAC_PI_3: Real = Real(Base::FRAC_PI_3);
    pub const FRAC_PI_4: Real = Real(Base::FRAC_PI_4);
    pub const FRAC_PI_6: Real = Real(Base::FRAC_PI_6);
    pub const FRAC_PI_8: Real = Real(Base::FRAC_PI_8);
    pub const FRAC_1_PI: Real = Real(Base::FRAC_1_PI);
    pub const FRAC_2_PI: Real = Real(Base::FRAC_2_PI);
    pub const FRAC_2_SQRT_PI: Real = Real(Base::FRAC_2_SQRT_PI);

    pub const SQRT_2: Real = Real(Base::SQRT_2);
    pub const FRAC_1_SQRT_2: Real = Real(Base::FRAC_1_SQRT_2);

    pub const E: Real = Real(Base::E);
    pub const LOG2_E: Real = Real(Base::LOG2_E);
    pub const LOG10_E: Real = Real(Base::LOG10_E);
    pub const LOG2_10: Real = Real(Base::LOG2_10);
    pub const LOG10_2: Real = Real(Base::LOG10_2);
    pub const LN_2: Real = Real(Base::LN_2);
    pub const LN_10: Real = Real(Base::LN_10);

    pub const fn leading_zeros(&self) -> u8 {
        self.0.leading_zeros() as u8
    }

    pub const fn leading_ones(&self) -> u8 {
        self.0.leading_ones() as u8
    }

    pub const fn from_bits(bits: i128) -> Real {
        Real(Base::from_bits(bits))
    }
}

impl<T> From<T> for Real
where
    T: ToFixed,
{
    fn from(value: T) -> Self {
        Real(Base::from_num(value))
    }
}

impl<T> Into<T> for Real
where
    T: FromFixed,
{
    fn into(self) -> T {
        self.0.to_num()
    }
}

impl Pow<Real> for Real {
    type Output = Real;

    fn pow(self, rhs: Real) -> Self::Output {
        exp(rhs * ln(self))
    }
}

impl_op_permutations!(
    types = [f64, f32, i64, i32, i16, i8, u64, u32, u16, u8, Real],
    exclude_specific = [(Real, Real)],
    exclude_permutations = [f64, f32, i64, i32, i16, i8, u64, u32, u16, u8],
    into = Real,
    out = Real,
    add = { lhs + rhs },
    sub = { lhs - rhs },
    mul = { lhs * rhs },
    div = { lhs / rhs },
    pow = { lhs.pow(rhs) },
    partial_eq = { lhs == rhs },
);

/* -------------------------------------------------------------------------- */

impl Display for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Binary for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Binary::fmt(&self.0, f)
    }
}

impl Octal for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Octal::fmt(&self.0, f)
    }
}

impl LowerHex for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl UpperHex for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

impl LowerExp for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        LowerExp::fmt(&self.0, f)
    }
}

impl UpperExp for Real {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        UpperExp::fmt(&self.0, f)
    }
}
