use std::{
    fmt::{Binary, Display, LowerExp, LowerHex, Octal, UpperExp, UpperHex},
    ops::{
        Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign,
        Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
    },
};

use derive_more::{From, Into};
use itertools::Itertools;
use nalgebra::{ComplexField, Field, RealField, SimdValue};
use num::{Float, FromPrimitive, Num, One, Signed, Zero, pow::Pow};
use simba::{scalar::SubsetOf, simd::PrimitiveSimdValue};

use crate::{core::util::impl_op_permutations, math::real::Real};

// Theres really no reason to use num_complex::Complex here because Real dosent implement Float anyway so we need to implement all the math manually
#[derive(Clone, Copy, PartialEq, Eq, Hash, From, Into, Debug)]
pub struct Complex {
    pub re: Real,
    pub im: Real,
}

#[macro_export]
macro_rules! c {
    ($r:literal, $i:literal) => {
        $crate::math::complex::Complex::new($crate::r!($r), $crate::r!($i))
    };
    ($r:literal) => {
        $crate::math::complex::Complex::new($crate::r!($r), $crate::r!(0))
    };
}

/* -------------------------------------------------------------------------- */

impl Add for Complex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self { re: self.re + rhs.re, im: self.im + rhs.im }
    }
}

impl AddAssign for Complex {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

impl Sub for Complex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self { re: self.re - rhs.re, im: self.im - rhs.im }
    }
}

impl SubAssign for Complex {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs
    }
}

impl Mul for Complex {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl MulAssign for Complex {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs
    }
}

impl Div for Complex {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / denom,
            im: (self.im * rhs.re - self.re * rhs.im) / denom,
        }
    }
}

impl DivAssign for Complex {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs
    }
}

impl Rem for Complex {
    type Output = Complex;

    fn rem(self, rhs: Self) -> Self::Output {
        Self { re: self.re % rhs.re, im: self.im % rhs.im }
    }
}

impl RemAssign for Complex {
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs
    }
}

impl Shl<u8> for Complex {
    type Output = Self;

    fn shl(self, rhs: u8) -> Self::Output {
        Self { re: self.re << rhs, im: self.im << rhs }
    }
}

impl ShlAssign<u8> for Complex {
    fn shl_assign(&mut self, rhs: u8) {
        *self = *self << rhs
    }
}

impl Shr<u8> for Complex {
    type Output = Self;

    fn shr(self, rhs: u8) -> Self::Output {
        Self { re: self.re >> rhs, im: self.im >> rhs }
    }
}

impl ShrAssign<u8> for Complex {
    fn shr_assign(&mut self, rhs: u8) {
        *self = *self >> rhs
    }
}

impl Neg for Complex {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self { re: -self.re, im: -self.im }
    }
}

impl Pow<Complex> for Complex {
    type Output = Self;

    fn pow(self, exp: Complex) -> Self::Output {
        (exp * self.ln()).exp()
    }
}

impl_op_permutations!(
    types = [f64, i64, Real, Complex],
    exclude_specific = [(Complex, Complex)],
    exclude_permutations = [f64, i64, Real],
    into = Complex,
    out = Complex,
    add = { lhs + rhs },
    sub = { lhs - rhs },
    mul = { lhs * rhs },
    div = { lhs / rhs },
    pow = { lhs.pow(rhs) },
    partial_eq = { lhs == rhs },
);

/* -------------------------------------------------------------------------- */

impl SubsetOf<Complex> for f64 {
    fn to_superset(&self) -> Complex {
        Complex::from_parts(*self, 0)
    }

    fn from_superset_unchecked(element: &Complex) -> Self {
        element.re.into()
    }

    fn is_in_subset(element: &Complex) -> bool {
        element.im == 0
    }
}

impl SubsetOf<Complex> for f32 {
    fn to_superset(&self) -> Complex {
        Complex::from_parts(*self, 0)
    }

    fn from_superset_unchecked(element: &Complex) -> Self {
        element.re.into()
    }

    fn is_in_subset(element: &Complex) -> bool {
        element.im == 0
    }
}

impl SubsetOf<Complex> for Complex {
    fn to_superset(&self) -> Complex {
        *self
    }

    fn from_superset_unchecked(element: &Complex) -> Self {
        *element
    }

    fn is_in_subset(element: &Complex) -> bool {
        true
    }
}

impl FromPrimitive for Complex {
    fn from_i64(n: i64) -> Option<Self> {
        Some(Self::from_parts(n, 0))
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(Self::from_parts(n, 0))
    }
}

impl SimdValue for Complex {
    const LANES: usize = Real::LANES;
    type Element = Complex;
    type SimdBool = bool;

    #[inline(always)]
    fn splat(val: Self::Element) -> Self {
        Complex { re: Real::splat(val.re), im: Real::splat(val.im) }
    }

    #[inline(always)]
    fn extract(&self, i: usize) -> Self::Element {
        Complex { re: self.re.extract(i), im: self.im.extract(i) }
    }

    #[inline(always)]
    unsafe fn extract_unchecked(&self, i: usize) -> Self::Element {
        unsafe {
            Complex {
                re: self.re.extract_unchecked(i),
                im: self.im.extract_unchecked(i),
            }
        }
    }

    #[inline(always)]
    fn replace(&mut self, i: usize, val: Self::Element) {
        self.re.replace(i, val.re);
        self.im.replace(i, val.im);
    }

    #[inline(always)]
    unsafe fn replace_unchecked(&mut self, i: usize, val: Self::Element) {
        unsafe {
            self.re.replace_unchecked(i, val.re);
            self.im.replace_unchecked(i, val.im);
        }
    }

    #[inline(always)]
    fn select(self, cond: Self::SimdBool, other: Self) -> Self {
        Complex {
            re: self.re.select(cond, other.re),
            im: self.im.select(cond, other.im),
        }
    }
}

impl PrimitiveSimdValue for Complex {}
impl Field for Complex {}

impl One for Complex {
    fn one() -> Self {
        c!(1)
    }
}

impl Zero for Complex {
    fn zero() -> Self {
        c!(0)
    }

    fn is_zero(&self) -> bool {
        self.re == 0 && self.im == 0
    }
}

impl Num for Complex {
    // TODO actual error handling
    type FromStrRadixErr = <Real as Num>::FromStrRadixErr;

    fn from_str_radix(
        str: &str,
        radix: u32,
    ) -> Result<Self, Self::FromStrRadixErr> {
        let iter = str.split("+");
        let mut parts = iter.collect_vec();

        if parts.len() > 2 {
            panic!("Invalid complex number str")
        }

        let mut im = parts.extract_if(.., |x| x.ends_with("i")).collect_vec();

        if im.len() > 1 {
            panic!("Cannot have more than one imaginary part")
        }

        let im = im.pop();
        let re = parts.pop();

        Ok(Self {
            re: re
                .map(|str| Real::from_str_radix(str, radix))
                .unwrap_or(Ok(Real::ZERO))?,
            im: im
                .map(|str| Real::from_str_radix(str, radix))
                .unwrap_or(Ok(Real::ZERO))?,
        })
    }
}

impl ComplexField for Complex {
    type RealField = Real;

    fn from_real(re: Self::RealField) -> Self {
        Complex::from_parts(re, 0)
    }

    fn real(self) -> Self::RealField {
        self.re
    }

    fn imaginary(self) -> Self::RealField {
        self.im
    }

    fn modulus(self) -> Self::RealField {
        self.modulus_squared().sqrt()
    }

    fn modulus_squared(self) -> Self::RealField {
        self.re * self.re + self.im * self.im
    }

    fn argument(self) -> Self::RealField {
        self.im.atan2(self.re)
    }

    fn norm1(self) -> Self::RealField {
        self.im.abs() + self.re.abs()
    }

    fn scale(self, factor: Self::RealField) -> Self {
        Self { re: self.re * factor, im: self.im * factor }
    }

    fn unscale(self, factor: Self::RealField) -> Self {
        Self { re: self.re / factor, im: self.im / factor }
    }

    fn floor(self) -> Self {
        Self { re: self.re.floor(), im: self.im.floor() }
    }

    fn ceil(self) -> Self {
        Self { re: self.re.ceil(), im: self.im.ceil() }
    }

    fn round(self) -> Self {
        Self { re: self.re.round(), im: self.im.round() }
    }

    fn trunc(self) -> Self {
        Self { re: self.re.trunc(), im: self.im.trunc() }
    }

    fn fract(self) -> Self {
        Self { re: self.re.fract(), im: self.im.fract() }
    }

    fn mul_add(self, a: Self, b: Self) -> Self {
        Self {
            re: self.re.mul_add(a.re, (-self.im).mul_add(a.im, b.re)),
            im: self.re.mul_add(a.im, self.im.mul_add(a.re, b.im)),
        }
    }

    fn abs(self) -> Self::RealField {
        self.modulus()
    }

    fn hypot(self, other: Self) -> Self::RealField {
        self.modulus_squared().hypot(other.modulus_squared())
    }

    fn recip(self) -> Self {
        let denom = self.modulus_squared();
        Self { re: self.re / denom, im: -self.im / denom }
    }

    fn conjugate(self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    fn sin(self) -> Self {
        Self {
            re: self.re.sin() * self.im.cosh(),
            im: self.re.cos() * self.im.sinh(),
        }
    }

    fn cos(self) -> Self {
        Self {
            re: self.re.cos() * self.im.cosh(),
            im: -self.re.sin() * self.im.sinh(),
        }
    }

    fn sin_cos(self) -> (Self, Self) {
        let cos_re = self.re.cos();
        let sin_re = self.re.sin();
        let cosh_im = self.im.cosh();
        let sinh_im = self.im.sinh();

        (
            Self { re: sin_re * cosh_im, im: cos_re * sinh_im },
            Self { re: cos_re * cosh_im, im: -sin_re * sinh_im },
        )
    }

    fn tan(self) -> Self {
        let (sin, cos) = self.sin_cos();

        sin / cos
    }

    fn asin(self) -> Self {
        let w = (1 - self * self).sqrt();
        (self * Complex::I + w).ln() * -Complex::I
    }

    fn acos(self) -> Self {
        Complex::FRAC_PI_2 - self.asin()
    }

    fn atan(self) -> Self {
        let iz = self * Complex::I;
        ((1 - iz).ln() - (1 + iz).ln()) * Complex::I >> 1
    }

    fn sinh_cosh(self) -> (Self, Self) {
        let (sinh_re, cosh_re) = self.re.sinh_cosh();
        let (sin_im, cos_im) = self.im.sin_cos();

        (
            Self { re: sinh_re * cos_im, im: cosh_re * sin_im },
            Self { re: cosh_re * cos_im, im: sinh_re * sin_im },
        )
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
        (self + (self * self + 1).sqrt()).ln()
    }

    fn acosh(self) -> Self {
        (self + (self + 1).sqrt() * (self - 1).sqrt()).ln()
    }

    fn atanh(self) -> Self {
        ((1 + self).ln() - (1 - self).ln()) >> 1
    }

    fn log(self, base: Self::RealField) -> Self {
        self.ln() / base.ln()
    }

    fn log2(self) -> Self {
        self.ln() / Real::LN_2
    }

    fn log10(self) -> Self {
        self.ln() / Real::LN_10
    }

    fn ln(self) -> Self {
        Self { re: self.modulus().ln(), im: self.im.atan2(self.re) }
    }

    fn ln_1p(self) -> Self {
        (self + 1).ln()
    }

    fn sqrt(self) -> Self {
        let r = self.modulus();
        let re = ((r + self.re) / 2).sqrt();
        let im = ((r - self.re) / 2).sqrt().copysign(self.im);

        Self { re, im }
    }

    fn exp(self) -> Self {
        let r = self.re.exp();

        Self { re: r * self.im.cos(), im: r * self.im.sin() }
    }

    fn exp2(self) -> Self {
        let θ = self.im * Real::LN_2;
        let r = self.re.exp2();

        Self { re: r * θ.cos(), im: r * θ.sin() }
    }

    fn exp_m1(self) -> Self {
        self.exp() - 1
    }

    fn powi(self, n: i32) -> Self {
        self.pow(n as i64)
    }

    fn powf(self, n: Self::RealField) -> Self {
        self.pow(n)
    }

    fn powc(self, n: Self) -> Self {
        self.pow(n)
    }

    fn cbrt(self) -> Self {
        let r = self.modulus().cbrt();
        let θ = self.argument() / 3;

        Self { re: r * θ.cos(), im: r * θ.sin() }
    }

    fn is_finite(&self) -> bool {
        true
    }

    fn try_sqrt(self) -> Option<Self> {
        Some(self.sqrt())
    }
}

/* -------------------------------------------------------------------------- */

impl Complex {
    pub const ZERO: Complex = c!(0);
    pub const ONE: Complex = c!(1);
    pub const I: Complex = c!(0, 1);

    pub const PI: Complex = Complex::new(Real::PI, Real::ZERO);
    pub const TAU: Complex = Complex::new(Real::TAU, Real::ZERO);
    pub const FRAC_PI_2: Complex = Complex::new(Real::FRAC_PI_2, Real::ZERO);
    pub const FRAC_PI_3: Complex = Complex::new(Real::FRAC_PI_3, Real::ZERO);
    pub const FRAC_PI_4: Complex = Complex::new(Real::FRAC_PI_4, Real::ZERO);
    pub const FRAC_PI_6: Complex = Complex::new(Real::FRAC_PI_6, Real::ZERO);
    pub const FRAC_PI_8: Complex = Complex::new(Real::FRAC_PI_8, Real::ZERO);
    pub const FRAC_1_PI: Complex = Complex::new(Real::FRAC_1_PI, Real::ZERO);
    pub const FRAC_2_PI: Complex = Complex::new(Real::FRAC_2_PI, Real::ZERO);
    pub const FRAC_2_SQRT_PI: Complex =
        Complex::new(Real::FRAC_2_SQRT_PI, Real::ZERO);

    pub const SQRT_2: Complex = Complex::new(Real::SQRT_2, Real::ZERO);
    pub const FRAC_1_SQRT_2: Complex =
        Complex::new(Real::FRAC_1_SQRT_2, Real::ZERO);

    pub const E: Complex = Complex::new(Real::E, Real::ZERO);
    pub const LOG2_E: Complex = Complex::new(Real::LOG2_E, Real::ZERO);
    pub const LOG10_E: Complex = Complex::new(Real::LOG10_E, Real::ZERO);
    pub const LOG2_10: Complex = Complex::new(Real::LOG2_10, Real::ZERO);
    pub const LOG10_2: Complex = Complex::new(Real::LOG10_2, Real::ZERO);
    pub const LN_2: Complex = Complex::new(Real::LN_2, Real::ZERO);
    pub const LN_10: Complex = Complex::new(Real::LN_10, Real::ZERO);

    pub const fn new(re: Real, im: Real) -> Self {
        Self { re, im }
    }

    pub fn from_parts(re: impl Into<Real>, im: impl Into<Real>) -> Self {
        Self { re: re.into(), im: im.into() }
    }

    pub fn is_integer(&self) -> bool {
        self.im.is_zero() && self.fract().is_zero()
    }

    pub fn is_real(&self) -> bool {
        self.im.is_zero()
    }

    pub fn is_imag(&self) -> bool {
        !self.im.is_zero() && self.re.is_zero()
    }

    pub fn as_integer(&self) -> Option<i64> {
        if self.is_integer() { Some(self.re.into()) } else { None }
    }

    pub fn as_real(&self) -> Option<Real> {
        if self.is_real() { Some(self.re) } else { None }
    }

    pub fn as_imag(&self) -> Option<Real> {
        if self.is_imag() { Some(self.im) } else { None }
    }
}

macro_rules! impl_complex_from_scalar {
    ($($t:ty),*) => {
        $(
        impl From<$t> for Complex {
            fn from(v: $t) -> Self {
                Complex::new(Real::from(v), Real::ZERO)
            }
        }
        )*
    };
}

impl_complex_from_scalar!(i64, i32, i16, i8, u64, u32, u16, u8, f64, f32, Real);

/* -------------------------------------------------------------------------- */

macro_rules! impl_complex_fmt {
    ($trait:ident) => {
        impl $trait for Complex {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                if self.re.is_zero() && !self.im.is_zero() {
                    $trait::fmt(&self.im, f)?;
                    write!(f, "i")
                } else if self.im.is_zero() {
                    $trait::fmt(&self.re, f)
                } else {
                    $trait::fmt(&self.re, f)?;
                    write!(f, " + ")?;

                    $trait::fmt(&self.im, f)?;
                    write!(f, "i")
                }
            }
        }
    };
}

impl_complex_fmt!(Display);
impl_complex_fmt!(Octal);
impl_complex_fmt!(Binary);
impl_complex_fmt!(LowerExp);
impl_complex_fmt!(UpperExp);
impl_complex_fmt!(LowerHex);
impl_complex_fmt!(UpperHex);
