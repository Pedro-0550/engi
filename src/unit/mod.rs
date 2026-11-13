use std::{
    f64::consts::LN_10,
    fmt::{Display, Write},
    hash::Hash,
    sync::{LazyLock, MappedRwLockReadGuard, RwLock, RwLockReadGuard},
};

use cranelift::codegen::gimli::write::UnitId;
use itertools::Itertools;
use num::{One, Rational, Signed, ToPrimitive, Zero, rational::Ratio};
use ordered_float::{OrderedFloat, Pow};
use thiserror::Error;

use crate::{
    core::{
        consed::{ConcurrentConsed, Consed},
        util::to_superscript,
        value::Value,
    },
    expr::Expr,
    id,
    unit::defs::UNITLESS,
};

pub mod defs;
pub mod ops;

#[cfg(test)]
mod test;

// TODO: design decision, should logarithmic units be treated like just views
// into linear data so if you say x * y and x and y are given in dB its actually +,
// or as normal where if you have x * y and x and y are log it just multiplies and gives dB^2?
//
// imo the first option since unit-specific logic is already required for operators with affine units
// so that the offset cancels out leaving only the scale
/* -------------------------------- CONSTANTS ------------------------------- */
type Composition = Vec<(Unit, Ratio<i8>)>;
static COMPOSITIONS: LazyLock<ConcurrentConsed<Composition, CompositionId>> =
    LazyLock::new(|| ConcurrentConsed::default());

id! { pub CompositionId }

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Error, Debug, Clone)]
pub enum DimensionalError {
    #[error("Tried to compare dimensionally incompatible expressions: {expr}")]
    IncompatibleComparison { expr: String },
    #[error("Tried to sum dimensionally incompatible expressions: {expr}")]
    IncompatibleSum { expr: String },
    #[error(
        "The pass and fail conditions of a piecewise have incompatible units: {expr}"
    )]
    IncompatibleConditional { expr: String },
    #[error(
        "Tried to apply a transcendental function to a dimensioned expression: {expr}"
    )]
    DimensionedTranscendental { expr: String },
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Dimensioned {
    fn dimension(&self) -> Result<Dimension, DimensionalError>;
    fn unit(&self) -> Result<Unit, DimensionalError>;
}

/* --------------------------------- STRUCTS -------------------------------- */

type Exponent = Ratio<i8>;

#[derive(Clone, Debug, Hash, Default)]
pub struct Quantity(Value, Unit);

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
#[allow(non_snake_case)]
pub struct Dimension {
    T: Ratio<i8>, // time
    L: Ratio<i8>, // length
    M: Ratio<i8>, // mass
    I: Ratio<i8>, // electric current
    θ: Ratio<i8>, // thermodynamic temperature
    J: Ratio<i8>, // luminous intensity
    N: Ratio<i8>, // amount of substance
}

/// This type is a *representation* of a unit.
///
/// You can have `m * s`, and `s * m`, which are mathematically exactly the same,
/// but are different representations of the `T^1 * L^1` dimension, and are not Eq.
///
/// This allows for using appropriate units for the context, like specifying a decay constant in s^-1 instead of Hz.
///
/// If you want to compare two different units by their dimensions, use [Unit.dimensional_eq].
/// If you want to compare them by their representations, use [Unit.repr_eq].
///
/// Checking can be done at the unit level, in which case you can't have an equation where one side is `s^-1 * m` and the other `Hz * m`, for example,
/// and in the dimension level, where that is perfectly fine because they are dimensionally the same, `T^-1 * L^1`.
///
/// Unit scale is always checked, so you cannot equal eV to J in an equation regardless of chosen level.
///
/// Order is preserved during compositions, and different orders of the same compositions are not Eq,
/// but order is ignored during checking, and only the equivalence is taken into account.
#[derive(PartialEq, Clone, Copy, Hash, Eq, Default)]
pub enum Unit {
    Base {
        symbol: &'static str,
        dimension: Dimension,
    },
    Derived {
        symbol: &'static str,
        base: &'static [(Unit, i8)],
    },
    Composed(CompositionId),
    /// Linear units are defined by a linear transformation on its base unit:
    /// `linaer = scale * base`
    Linear {
        symbol: &'static str,
        base: &'static Unit,
        scale: OrderedFloat<f64>,
    },
    /// Affine units are defined by an affine transformation on its base unit:
    /// `affine = (scale * base) + offset`
    // TODO: Affine differences cancel out the offset, Leaving a linear with the scaled equal to affine's scale
    Affine {
        symbol: &'static str,
        base: &'static Unit,
        offset: OrderedFloat<f64>,
        scale: OrderedFloat<f64>,
    },
    /// Logarithmic units are defined by a log operation on its reference unit:
    /// `logarithmic = log(base, ref)`
    /// such that `ref = base^logarithmic`
    // Function syntax for setting refernces: dB(V), dB(mW)
    Logarithmic {
        symbol: &'static str,
        reference: &'static Unit,
        base: OrderedFloat<f64>,
    },
    #[default]
    Unitless,
}

/* ---------------------------------- IMPLS --------------------------------- */

const impl Default for Dimension {
    fn default() -> Self {
        Self {
            T: Ratio::ZERO,
            L: Ratio::ZERO,
            M: Ratio::ZERO,
            I: Ratio::ZERO,
            θ: Ratio::ZERO,
            J: Ratio::ZERO,
            N: Ratio::ZERO,
        }
    }
}

impl Dimension {
    pub const DIMENSIONLESS: Dimension = Dimension::default();

    // SI base dimensions

    pub const TIME: Dimension =
        Dimension { T: Ratio::ONE, ..Default::default() };

    pub const LENGTH: Dimension =
        Dimension { L: Ratio::ONE, ..Default::default() };

    pub const MASS: Dimension =
        Dimension { M: Ratio::ONE, ..Default::default() };

    pub const ELECTRIC_CURRENT: Dimension =
        Dimension { I: Ratio::ONE, ..Default::default() };

    pub const TEMPERATURE: Dimension =
        Dimension { θ: Ratio::ONE, ..Default::default() };

    pub const LUMINOUS_INTENSITY: Dimension =
        Dimension { J: Ratio::ONE, ..Default::default() };

    pub const AMOUNT_OF_SUBSTANCE: Dimension =
        Dimension { N: Ratio::ONE, ..Default::default() };
}

impl Quantity {
    pub const fn new(value: Value, unit: Unit) -> Self {
        Self(value, unit)
    }

    pub const ZERO: Self = Self(Value::ZERO, UNITLESS);

    pub fn realize(&self) -> [Quantity; 2] {
        let [re, im] = self.0.realize();

        [re * self.unit(), im * self.unit()]
    }

    /// Normalizes this quantity to its non-scaled form.
    /// If this quantity is given in a scaled unit such as eV, it will convert to Joule and scale its value appropriately.
    /// This will be done recursively until a non-scaled unit is reached.
    /// If this quantity is given in a composition containing scaled units, all scaled units will be reduced to non-scaled form.
    /// If this quantity is logarithmic, it will be converted to the linear form.
    /// For affine quantities, their scaling and offset will be applied.
    ///
    /// If it does not contain any scale, self is returned instead.
    ///
    /// For example, `10 eV` becomes `1.602176634e-18 J`.
    /// This also simplifies compositions of scaled but dimensionally equal units: `10 eV/J` becomes `1.602176634e-18 (unitless)`
    pub fn normalize(self) -> Quantity {
        let Quantity(mut current_val, mut current_unit) = self;

        loop {
            let normalized = match current_unit {
                Unit::Linear { base, scale, .. } => {
                    current_val *= scale.0;
                    *base
                }
                Unit::Composed(id) => {
                    let mut terms = COMPOSITIONS.con_get(id).unwrap().clone();
                    for (unit, exp) in &mut terms {
                        if let Unit::Linear { base, scale, .. } = *unit {
                            *unit = *base;
                            current_val *= scale.pow(exp.to_f64().unwrap()).0;
                        }
                    }

                    Unit::new_composition(terms)
                }
                Unit::Logarithmic { reference, base, .. } => {
                    current_val = base.0.pow(current_val);

                    *reference
                }
                Unit::Affine { base, offset, scale, .. } => {
                    current_val *= scale.0;
                    current_val += offset.0;
                    *base
                }
                _ => current_unit,
            };

            if normalized == current_unit {
                break;
            }

            current_unit = normalized;
        }

        Quantity(current_val, current_unit)
    }

    /// Converts this quantity to the nearest SI prefix of its unit
    pub fn prefixalize(&self) -> Self {
        if self.value().is_zero() {
            return self.clone();
        }

        let mag = self
            .value()
            .as_scalar_real()
            .expect("Prefixalize can only be used on real scalars")
            .abs()
            .log10();
        let rounded_mag = (mag as i32).div_floor(3) * 3;

        let scale = 10.0f64.pow(rounded_mag);

        fn prefixalize_inner(unit: Unit, rounded_mag: i32) -> Option<Unit> {
            let scale = 10.0f64.pow(rounded_mag);
            if rounded_mag == 0 {
                return Some(unit);
            }

            let prefix = match rounded_mag {
                -30 => "q",
                -27 => "r",
                -24 => "y",
                -21 => "z",
                -18 => "a",
                -15 => "f",
                -12 => "p",
                -9 => "n",
                -6 => "µ",
                -3 => "m",
                0 => "",
                3 => "k",
                6 => "M",
                9 => "G",
                12 => "T",
                15 => "P",
                18 => "E",
                21 => "Z",
                24 => "Y",
                27 => "R",
                30 => "Q",
                _ => return None,
            };

            match unit {
                Unit::Base { symbol, dimension } => Some(Unit::Linear {
                    symbol: format!("{}{}", prefix, symbol).leak(),
                    base: Box::leak(Box::new(unit)),
                    scale: OrderedFloat(scale),
                }),
                Unit::Derived { symbol, base } => Some(Unit::Linear {
                    symbol: format!("{}{}", prefix, symbol).leak(),
                    base: Box::leak(Box::new(unit)),
                    scale: OrderedFloat(scale),
                }),
                Unit::Composed(composition_id) => {
                    let mut comp =
                        COMPOSITIONS.con_get(composition_id).unwrap().clone();

                    if *comp[0].1.denom() != 1 {
                        todo!()
                    }

                    comp[0].0 = prefixalize_inner(
                        comp[0].0,
                        rounded_mag * *comp[0].1.numer() as i32,
                    )?;

                    Some(Unit::Composed(COMPOSITIONS.insert(comp)))
                }
                Unit::Linear { symbol, base, scale: prev } => {
                    Some(Unit::Linear {
                        symbol: format!("{}{}", prefix, symbol).leak(),
                        // TODO: as obvious, this is terrible. We want refcounted units probably
                        base: Box::leak(Box::new(unit)),
                        scale: OrderedFloat(scale * prev.0),
                    })
                }
                Unit::Affine { symbol, base, offset, scale } => return None,
                Unit::Logarithmic { symbol, reference, base } => return None,
                Unit::Unitless => return None,
            }
        }

        let unit = prefixalize_inner(self.unit(), rounded_mag).unwrap();
        let value = self.0.clone() / scale;

        Quantity(value, unit)
    }

    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }

    pub fn unit(&self) -> Unit {
        self.1
    }
}

fn fold_composition(composition: &mut Vec<(Unit, Exponent)>) {
    let mut i = 0;

    while i < composition.len() {
        let (unit_i, _) = composition[i];

        let mut j = i + 1;

        while j < composition.len() {
            let (unit_j, exp_j) = composition[j];
            if unit_j == unit_i {
                composition[i].1 += exp_j;
                composition.remove(j);
            } else {
                j += 1;
            }
        }

        if composition[i].1.is_zero() {
            composition.remove(i);
        } else {
            i += 1;
        }
    }
}

impl<F> From<F> for Unit
where
    F: Fn(Unit) -> Unit,
{
    fn from(f: F) -> Self {
        f(UNITLESS)
    }
}

impl Unit {
    /// Checks if two unit's *dimensions* are equivalent.
    /// For example, `s^-1 * m` and `m * Hz` are equivalent at the dimensional level, but not in the representational level.
    /// In contrast, `s^-1 * m` and `m * s^-1` are equivalent in both worlds.
    pub fn dimensional_eq(self, rhs: Unit) -> bool {
        if let Ok(self_dim) = self.dimension()
            && let Ok(rhs_dim) = rhs.dimension()
        {
            self_dim == rhs_dim
        } else {
            false
        }
    }

    /// Checks if two unit's *representations* are equivalent.
    /// For example, `s^-1 * m`, `m * Hz` and `km hr^-1` are equivalent at the dimensional level, but not in the representational level.
    /// In contrast, `s^-1 * m` and `m * s^-1` are equivalent in both worlds.
    pub fn repr_eq(self, rhs: Unit) -> bool {
        match (self, rhs) {
            (Unit::Composed(lhs_id), Unit::Composed(rhs_id)) => {
                let lhs_comp = COMPOSITIONS.con_get(lhs_id).unwrap();
                let rhs_comp = COMPOSITIONS.con_get(rhs_id).unwrap();
                lhs_comp.iter().all(|x| rhs_comp.contains(x))
            }
            (..) => self == rhs,
        }
    }

    /// Returns true if this unit is atomic, that is, represented by a single base unit or derived unit, and an optional multiplier.
    /// False if its composed, including exponentiation of a single unit.
    fn is_atomic(&self) -> bool {
        match self {
            Unit::Base { .. }
            | Unit::Derived { .. }
            | Unit::Affine { .. }
            | Unit::Logarithmic { .. } => true,

            Unit::Linear { base, .. } => base.is_atomic(),

            Unit::Unitless | Unit::Composed { .. } => false,
        }
    }

    fn new_composition(mut comp: Composition) -> Unit {
        fold_composition(&mut comp);

        if comp.is_empty() {
            return UNITLESS;
        } else if comp.len() == 1
            && let Some((unit, exp)) = comp.first()
            && unit.is_atomic()
            && *&exp.is_one()
        {
            return *unit;
        }

        Unit::Composed(COMPOSITIONS.insert(comp))
    }
}

impl Dimensioned for Unit {
    fn dimension(&self) -> Result<Dimension, DimensionalError> {
        let mut current_dim = Dimension::DIMENSIONLESS;

        match self {
            Unit::Base { dimension, .. } => {
                current_dim *= *dimension;
            }
            Unit::Derived { base, .. } => {
                for (unit, exp) in *base {
                    current_dim *= unit.dimension()?.pow(Ratio::new(*exp, 1));
                }
            }
            Unit::Affine { base, .. } => {
                current_dim *= base.dimension()?;
            }
            Unit::Logarithmic { reference, .. } => {
                current_dim *= reference.dimension()?;
            }
            Unit::Linear { base, .. } => {
                current_dim *= base.dimension()?;
            }
            Unit::Composed(id) => {
                for (unit, exp) in COMPOSITIONS.con_get(*id).unwrap() {
                    current_dim *= unit.dimension()?.pow(*exp);
                }
            }
            Unit::Unitless => (),
        }

        Ok(current_dim)
    }

    fn unit(&self) -> Result<Unit, DimensionalError> {
        Ok(*self)
    }
}

impl Dimensioned for Quantity {
    fn dimension(&self) -> Result<Dimension, DimensionalError> {
        self.1.dimension()
    }

    fn unit(&self) -> Result<Unit, DimensionalError> {
        Ok(self.unit())
    }
}

impl Dimensioned for Expr {
    fn dimension(&self) -> Result<Dimension, DimensionalError> {
        self.unit().and_then(|u| u.dimension())
    }

    fn unit(&self) -> Result<Unit, DimensionalError> {
        // match self.node() {
        //     Unit::Symbol(symbol) => Ok(symbol.unit()),
        //     Unit::Constant(constant) => Ok(constant.quantity().unit()),
        //     Unit::Quantity(quantity) => Ok(quantity.unit()),
        //     Unit::Add(exprs) | Unit::Min(exprs) | Unit::Max(exprs) => {
        //         let first = exprs.first().unwrap().unit()?;

        //         for expr in exprs.iter().skip(1) {
        //             if expr.unit()? != first {
        //                 return Err(
        //                     if self.node().is_max() | self.node().is_min() {
        //                         DimensionalError::IncompatibleComparison {
        //                             expr: self.into(),
        //                         }
        //                     } else {
        //                         DimensionalError::IncompatibleSum {
        //                             expr: self.into(),
        //                         }
        //                     },
        //                 );
        //             }
        //         }

        //         Ok(first)
        //     }

        //     Unit::Mul(exprs) => Ok(exprs
        //         .iter()
        //         .try_fold(Unit::Unitless, |acc, x| Ok(acc * x.unit()?))?),
        //     Unit::Sin(expr)
        //     | Unit::Cos(expr)
        //     | Unit::Tan(expr)
        //     | Unit::Asin(expr)
        //     | Unit::Acos(expr)
        //     | Unit::Atan(expr)
        //     | Unit::Sinh(expr)
        //     | Unit::Cosh(expr)
        //     | Unit::Tanh(expr)
        //     | Unit::Asinh(expr)
        //     | Unit::Acosh(expr)
        //     | Unit::Atanh(expr) => {
        //         if expr.dimension()? != DIMENSIONLESS {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //         } else {
        //             Ok(Unit::Unitless)
        //         }
        //     }
        //     Unit::Arg(expr) => Ok(rad),
        //     Unit::Conj(expr) => expr.unit(),
        //     Unit::Norm(expr) => expr.unit(),
        //     Unit::Sign(expr) => expr.unit(),
        //     Unit::Real(expr) => expr.unit(),
        //     Unit::Imag(expr) => expr.unit(),
        //     Unit::Pow { exp, base } => {
        //         if exp.dimension()? != DIMENSIONLESS {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //             // TODO: Fix
        //         } else if let Some(qty) = exp
        //             .node()
        //             .as_quantity()
        //             .and_then(|qty| qty.value().as_scalar_integer())
        //         {
        //             Ok(base.unit()?.pow(qty))
        //         } else {
        //             Ok(Unit::Unitless)
        //         }
        //     }
        //     Unit::Log { base, arg } => {
        //         if arg.dimension()? != DIMENSIONLESS {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //         } else {
        //             Ok(Unit::Unitless)
        //         }
        //     }
        //     Unit::Atan2 { a, b } => {
        //         if a.dimension()? != DIMENSIONLESS
        //             && b.dimension()? != DIMENSIONLESS
        //         {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //         } else {
        //             Ok(rad)
        //         }
        //     }
        //     Unit::Matrix(matrix) => todo!(),
        //     Unit::Piecewise { arms, default } => todo!(),
        //     Unit::Transpose(expr) => expr.unit(),
        //     Unit::Det(expr) => todo!(),
        //     Unit::Rank(expr) => todo!(),
        //     Unit::Trace(expr) => todo!(),
        // }
        // }
        todo!()
    }
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Base { symbol, dimension } => f
                .debug_struct("Base")
                .field("symbol", symbol)
                .field("dimension", dimension)
                .finish(),
            Self::Derived { symbol, base } => f
                .debug_struct("Derived")
                .field("symbol", symbol)
                .field("base", base)
                .finish(),
            Self::Composed(id) => f
                .debug_tuple("Composed")
                .field(COMPOSITIONS.con_get(*id).unwrap())
                .finish(),
            Self::Linear { symbol, base, scale } => f
                .debug_struct("Linear")
                .field("symbol", symbol)
                .field("base", base)
                .field("scale", scale)
                .finish(),
            Self::Affine { symbol, base, offset, scale } => f
                .debug_struct("Affine")
                .field("symbol", symbol)
                .field("base", base)
                .field("offset", offset)
                .field("scale", scale)
                .finish(),
            Self::Logarithmic { symbol, reference, base } => f
                .debug_struct("Logarithmic")
                .field("symbol", symbol)
                .field("reference", reference)
                .field("base", base)
                .finish(),
            Self::Unitless => write!(f, "Unitless"),
        }
    }
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unit::Base { symbol, .. }
            | Unit::Derived { symbol, .. }
            | Unit::Linear { symbol, .. }
            | Unit::Affine { symbol, .. } => f.write_str(symbol),
            Unit::Unitless => Ok(()),
            Unit::Logarithmic { symbol, reference, .. } => {
                f.write_str(symbol)?;

                if **reference != UNITLESS {
                    f.write_char('(')?;
                    reference.fmt(f)?;
                    f.write_char(')')?;
                }

                Ok(())
            }
            Unit::Composed(id) => {
                let terms = COMPOSITIONS.con_get(*id).unwrap();

                fn fmt_term(
                    f: &mut std::fmt::Formatter<'_>,
                    unit: Unit,
                    exp: Exponent,
                ) -> std::fmt::Result {
                    match (exp.numer(), exp.denom()) {
                        (1, denom @ 2..=4) => {
                            match denom {
                                2 => write!(f, "√")?,
                                3 => write!(f, "∛")?,
                                4 => write!(f, "∜")?,
                                _ => unreachable!(),
                            }
                            unit.fmt(f)?;
                        }
                        (numer, denom) if numer.abs() != denom.abs() => {
                            unit.fmt(f)?;

                            if *denom == 1 {
                                f.write_str(&to_superscript(*numer as i64))?;
                            } else if denom != numer {
                                write!(f, "^{:.3}", exp.to_f64().unwrap())?;
                            }
                        }
                        _ => unit.fmt(f)?,
                    }

                    Ok(())
                }

                let (num, denom): (Vec<_>, Vec<_>) =
                    terms.iter().partition(|(_, exp)| *exp > Ratio::ZERO);

                match (num.len(), denom.len()) {
                    (1.., ..) => {
                        for (i, (unit, exp)) in num.iter().enumerate() {
                            fmt_term(f, *unit, *exp)?;

                            if i < num.len() - 1 {
                                f.write_str("·")?;
                            }
                        }

                        if denom.len() == 0 {
                            return Ok(());
                        }

                        let parenthesize_denom = denom.len() > 1;

                        f.write_str("/")?;

                        if parenthesize_denom {
                            f.write_str("(")?;
                        }

                        for (i, (unit, exp)) in denom.iter().enumerate() {
                            fmt_term(f, *unit, *exp)?;

                            if i < denom.len() - 1 {
                                f.write_str("·")?;
                            }
                        }

                        if parenthesize_denom {
                            f.write_str(")")?;
                        }
                    }
                    (0, 1..) => {
                        for (i, (unit, exp)) in denom.iter().enumerate() {
                            fmt_term(f, *unit, *exp)?;

                            if i < num.len() - 1 {
                                f.write_str("·")?;
                            }
                        }
                    }
                    _ => unreachable!(),
                }

                Ok(())
            }
        }
    }
}

impl Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)?;

        if self.1 != UNITLESS {
            f.write_char(' ')?;
            self.1.fmt(f)?;
        }

        Ok(())
    }
}

impl Eq for Quantity {}

impl PartialEq for Quantity {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_zero() && other.0.is_zero() {
            true
        } else {
            self.0 == other.0 && self.1 == other.1
        }
    }
}
