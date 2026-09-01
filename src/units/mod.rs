use std::{
    fmt::{Display, Write},
    hash::Hash,
};

use num::Complex;
use ordered_float::OrderedFloat;
use thiserror::Error;

use crate::{
    core::{
        interned::{Handle, Interned},
        util::to_superscript,
        value::{Scalar, Value},
    },
    expr::Expr,
    units::isq::DIMENSIONLESS,
};

pub mod isq;
pub mod non_si;
pub mod ops;
pub mod si;

#[cfg(test)]
mod test;

/* -------------------------------- CONSTANTS ------------------------------- */

type Composition = Vec<(Unit, i8)>;

static COMPOSITIONS: Interned<Composition> = Interned::new();

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Error, Debug, Clone)]
pub enum DimensionalAnalysisError {
    #[error(
        "Tried to sum dimensionally incompatible expressions: {lhs} and {rhs}"
    )]
    IncompatibleSum { lhs: Expr, rhs: Expr },
    #[error(
        "Tried to apply a transcendental function to a dimensioned expression: {expr}"
    )]
    DimensionedTranscendental { expr: Expr },
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Dimensioned {
    fn analyze(&self) -> Result<Dimension, DimensionalAnalysisError>;
}

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(PartialEq, Clone, Debug, Hash)]
pub struct Quantity(Value, Unit);

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
#[allow(non_snake_case)]
pub struct Dimension {
    T: i8, // time
    L: i8, // length
    M: i8, // mass
    I: i8, // electric current
    Θ: i8, // thermodynamic temperature
    J: i8, // luminous intensity
    N: i8, // amount of substance
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
#[derive(PartialEq, Clone, Copy, Debug, Hash, Eq)]
#[allow(non_snake_case)]
pub enum Unit {
    Base {
        symbol: &'static str,
        dimension: Dimension,
    },
    Derived {
        symbol: &'static str,
        base: &'static [(Unit, i8)],
    },
    Composed(Handle<Composition>),
    Scaled {
        symbol: &'static str,
        base: &'static Unit,
        scale: OrderedFloat<f64>,
    },
    Unitless,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Quantity {
    pub const ZERO: Self = Self(Value::Scalar(Scalar::ZERO), Unit::Unitless);

    /// Normalizes this quantity to its non-scaled form.
    /// If this quantity is given in a scaled unit such as eV, it will convert to Joule and scale its value appropriately.
    /// This will be done recursively until a non-scaled unit is reached.
    /// If this quantity is given in a composition containing scaled units, all scaled units will be reduced to non-scaled form.
    ///
    /// If it does not contain any scale, self is returned instead.
    ///
    /// For example, `10 eV` becomes `1.602176634e-18 J`.
    /// This also simplifies compositions of scaled but dimensionally equal units: `10 eV/J` becomes `1.602176634e-18 (unitless)`
    pub fn normalize(self) -> Quantity {
        let Quantity(mut current_val, mut current_unit) = self;

        loop {
            let normalized = match current_unit {
                Unit::Scaled { base, scale, .. } => {
                    current_val *= scale.0;
                    *base
                }
                Unit::Composed(id) => {
                    let mut composition = COMPOSITIONS.get_cloned(id).unwrap();

                    for (unit, exp) in composition.iter_mut() {
                        if let Unit::Scaled { base, scale, .. } = *unit {
                            *unit = *base;
                            current_val *= scale.powi(*exp as i32);
                        }
                    }

                    Unit::new_composition(composition)
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

    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn unit(&self) -> Unit {
        return self.1;
    }
}

fn fold_composition(composition: &mut Composition) {
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

        if composition[i].1 == 0 {
            composition.remove(i);
        } else {
            i += 1;
        }
    }
}

impl Unit {
    /// Checks if two unit's *dimensions* are equivalent.
    /// For example, `s^-1 * m` and `m * Hz` are equivalent at the dimensional level, but not in the representational level.
    /// In contrast, `s^-1 * m` and `m * s^-1` are equivalent in both worlds.
    pub fn dimensional_eq(self, rhs: Unit) -> bool {
        if let Ok(self_dim) = self.analyze()
            && let Ok(rhs_dim) = rhs.analyze()
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
            (Unit::Composed(self_id), Unit::Composed(rhs_id)) => {
                let self_comp = COMPOSITIONS.get_cloned(self_id).unwrap();
                let rhs_comp = COMPOSITIONS.get_cloned(rhs_id).unwrap();

                self_comp.iter().all(|x| rhs_comp.contains(x))
            }
            (..) => self == rhs,
        }
    }

    /// Returns true if this unit is atomic, that is, represented by a single base unit or derived unit, and an optional multiplier.
    /// False if its composed, including exponentiation of a single unit.
    /// TODO: recursively check scaled unit's atomicity
    fn is_atomic(&self) -> bool {
        match self {
            Self::Base { .. } | Self::Derived { .. } => true,

            Self::Scaled { base, .. } => base.is_atomic(),

            Self::Unitless | Self::Composed(_) => false,
        }
    }

    fn new_composition(mut comp: Composition) -> Self {
        fold_composition(&mut comp);

        if comp.len() == 0 {
            return Unit::Unitless;
        } else if comp.len() == 1
            && let Some((unit, exp)) = comp.first()
            && unit.is_atomic()
            && *exp == 1
        {
            return *unit;
        }

        if let Some(existing) = COMPOSITIONS.handle_of(&comp) {
            return Unit::Composed(existing);
        }

        let id = COMPOSITIONS.insert(comp);

        Unit::Composed(id)
    }
}

impl Dimensioned for Unit {
    fn analyze(&self) -> Result<Dimension, DimensionalAnalysisError> {
        let mut current_dim = DIMENSIONLESS;

        match self {
            Unit::Base { dimension, .. } => {
                current_dim *= *dimension;
            }
            Unit::Derived { base, .. } => {
                for (unit, exp) in *base {
                    current_dim *= unit.analyze()?.pow(*exp);
                }
            }
            Unit::Scaled { base, .. } => {
                current_dim *= base.analyze()?;
            }
            Unit::Composed(id) => {
                for (unit, exp) in COMPOSITIONS.get_cloned(*id).unwrap() {
                    current_dim *= unit.analyze()?.pow(exp);
                }
            }
            Unit::Unitless => (),
        }

        Ok(current_dim)
    }
}

impl Dimensioned for Quantity {
    fn analyze(&self) -> Result<Dimension, DimensionalAnalysisError> {
        self.1.analyze()
    }
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unit::Base { symbol, .. }
            | Unit::Derived { symbol, .. }
            | Unit::Scaled { symbol, .. } => f.write_str(symbol),
            Unit::Unitless => Ok(()),
            Unit::Composed(id) => {
                let comp = COMPOSITIONS.get_cloned(*id).unwrap();
                let (num, denom): (Vec<_>, Vec<_>) =
                    comp.iter().partition(|(_, exp)| *exp > 0);

                match (num.len(), denom.len()) {
                    (1.., ..) => {
                        for (i, (unit, exp)) in num.iter().enumerate() {
                            unit.fmt(f)?;

                            if *exp != 1 {
                                f.write_str(&to_superscript(*exp as i32))?;
                            }

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
                            unit.fmt(f)?;

                            if *exp != -1 {
                                f.write_str(&to_superscript(exp.abs() as i32))?;
                            }

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
                            unit.fmt(f)?;

                            f.write_str(&to_superscript(*exp as i32))?;

                            if i < num.len() - 1 {
                                f.write_str(" * ")?;
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

        if self.1 != Unit::Unitless {
            f.write_char(' ')?;
            self.1.fmt(f)?;
        }

        Ok(())
    }
}

impl Eq for Quantity {}
