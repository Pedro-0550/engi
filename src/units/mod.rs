use std::{
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
    units::defs::{DEFAULT_UNITS, UNITLESS},
};

pub mod defs;
pub mod ops;

#[cfg(test)]
mod test;

/* -------------------------------- CONSTANTS ------------------------------- */

static DAG: LazyLock<ConcurrentConsed<Node, NodeId>> = LazyLock::new(|| {
    let mut c = ConcurrentConsed::<Node, NodeId>::default();
    for (i, node) in DEFAULT_UNITS.into_iter().enumerate() {
        assert_eq!(c.insert(node).0 as usize, i);
    }
    c
});

id! { pub NodeId }

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

#[derive(PartialEq, Clone, Debug, Hash, Eq, Default)]
#[allow(non_snake_case)]
enum Node {
    Base {
        symbol: &'static str,
        dimension: Dimension,
    },
    Derived {
        symbol: &'static str,
        base: &'static [(NodeId, i8)],
    },
    Composed {
        terms: Vec<(NodeId, Exponent)>,
    },
    /// Linear units are defined by a linear transformation on its base unit:
    /// `linaer = scale * base`
    Linear {
        symbol: &'static str,
        base: NodeId,
        scale: OrderedFloat<f64>,
    },
    /// Affine units are defined by an affine transformation on its base unit:
    /// `affine = (scale * base) + offset`
    Affine {
        symbol: &'static str,
        base: NodeId,
        offset: OrderedFloat<f64>,
        scale: OrderedFloat<f64>,
    },
    /// Logarithmic units are defined by a log operation on its reference unit:
    /// `logarithmic = log(base, ref)`
    /// such that `ref = base^logarithmic`
    // Function syntax for setting refernces: dB(V), dB(mW)
    Logarithmic {
        symbol: &'static str,
        reference: NodeId,
        base: OrderedFloat<f64>,
    },
    #[default]
    Unitless,
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
pub type Unit = NodeId;

// Unitless's index must be 0 so that the default is Unitless
const _: () = assert!(UNITLESS.0 == 0);

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

impl NodeId {
    fn fetch(&self) -> &Node {
        DAG.con_get(*self).unwrap()
        // RwLockReadGuard::map(DAG.read().unwrap(), |x| x.get(*self).unwrap())
    }
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
            let normalized = match *current_unit.fetch() {
                Node::Linear { base, scale, .. } => {
                    current_val *= scale.0;
                    base
                }
                Node::Composed { ref terms } => {
                    let mut terms = terms.clone();
                    for (node, exp) in &mut terms {
                        if let Node::Linear { base, scale, .. } =
                            *node.clone().fetch()
                        {
                            *node = base;
                            current_val *= scale.pow(exp.to_f64().unwrap()).0;
                        }
                    }

                    Unit::new_composition(terms)
                }
                Node::Logarithmic { reference, base, .. } => {
                    current_val = base.0.pow(current_val);

                    reference
                }
                Node::Affine { base, offset, scale, .. } => {
                    current_val *= scale.0;
                    current_val += offset.0;
                    base
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
        self.fetch().repr_eq(&rhs.fetch())
    }

    /// Returns true if this unit is atomic, that is, represented by a single base unit or derived unit, and an optional multiplier.
    /// False if its composed, including exponentiation of a single unit.
    fn is_atomic(&self) -> bool {
        self.fetch().is_atomic()
    }

    fn new_composition(mut comp: Vec<(NodeId, Ratio<i8>)>) -> NodeId {
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

        DAG.insert(Node::Composed { terms: comp })
    }
}

impl Node {
    pub fn repr_eq(&self, rhs: &Node) -> bool {
        match (self, rhs) {
            (Node::Composed { terms: lhs }, Node::Composed { terms: rhs }) => {
                lhs.iter().all(|x| rhs.contains(x))
            }
            (..) => self == rhs,
        }
    }

    fn is_atomic(&self) -> bool {
        match self {
            Node::Base { .. }
            | Node::Derived { .. }
            | Node::Affine { .. }
            | Node::Logarithmic { .. } => true,

            Node::Linear { base, .. } => base.is_atomic(),

            Node::Unitless | Node::Composed { .. } => false,
        }
    }
}

impl Dimensioned for Unit {
    fn dimension(&self) -> Result<Dimension, DimensionalError> {
        let mut current_dim = Dimension::DIMENSIONLESS;

        match &*self.fetch() {
            Node::Base { dimension, .. } => {
                current_dim *= *dimension;
            }
            Node::Derived { base, .. } => {
                for (unit, exp) in *base {
                    current_dim *= unit.dimension()?.pow(Ratio::new(*exp, 1));
                }
            }
            Node::Affine { base, .. } => {
                current_dim *= base.dimension()?;
            }
            Node::Logarithmic { reference, .. } => {
                current_dim *= reference.dimension()?;
            }
            Node::Linear { base, .. } => {
                current_dim *= base.dimension()?;
            }
            Node::Composed { terms } => {
                for (unit, exp) in terms {
                    current_dim *= unit.dimension()?.pow(*exp);
                }
            }
            Node::Unitless => (),
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
        //     Node::Symbol(symbol) => Ok(symbol.unit()),
        //     Node::Constant(constant) => Ok(constant.quantity().unit()),
        //     Node::Quantity(quantity) => Ok(quantity.unit()),
        //     Node::Add(exprs) | Node::Min(exprs) | Node::Max(exprs) => {
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

        //     Node::Mul(exprs) => Ok(exprs
        //         .iter()
        //         .try_fold(Unit::Unitless, |acc, x| Ok(acc * x.unit()?))?),
        //     Node::Sin(expr)
        //     | Node::Cos(expr)
        //     | Node::Tan(expr)
        //     | Node::Asin(expr)
        //     | Node::Acos(expr)
        //     | Node::Atan(expr)
        //     | Node::Sinh(expr)
        //     | Node::Cosh(expr)
        //     | Node::Tanh(expr)
        //     | Node::Asinh(expr)
        //     | Node::Acosh(expr)
        //     | Node::Atanh(expr) => {
        //         if expr.dimension()? != DIMENSIONLESS {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //         } else {
        //             Ok(Unit::Unitless)
        //         }
        //     }
        //     Node::Arg(expr) => Ok(rad),
        //     Node::Conj(expr) => expr.unit(),
        //     Node::Norm(expr) => expr.unit(),
        //     Node::Sign(expr) => expr.unit(),
        //     Node::Real(expr) => expr.unit(),
        //     Node::Imag(expr) => expr.unit(),
        //     Node::Pow { exp, base } => {
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
        //     Node::Log { base, arg } => {
        //         if arg.dimension()? != DIMENSIONLESS {
        //             Err(DimensionalError::DimensionedTranscendental {
        //                 expr: self.into(),
        //             })
        //         } else {
        //             Ok(Unit::Unitless)
        //         }
        //     }
        //     Node::Atan2 { a, b } => {
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
        //     Node::Matrix(matrix) => todo!(),
        //     Node::Piecewise { arms, default } => todo!(),
        //     Node::Transpose(expr) => expr.unit(),
        //     Node::Det(expr) => todo!(),
        //     Node::Rank(expr) => todo!(),
        //     Node::Trace(expr) => todo!(),
        // }
        // }
        todo!()
    }
}

impl Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fetch().fmt(f)
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Base { symbol, .. }
            | Node::Derived { symbol, .. }
            | Node::Linear { symbol, .. }
            | Node::Affine { symbol, .. } => f.write_str(symbol),
            Node::Unitless => Ok(()),
            Node::Logarithmic { symbol, reference, .. } => {
                f.write_str(symbol)?;

                if *reference != UNITLESS {
                    f.write_char('(')?;
                    reference.fetch().fmt(f)?;
                    f.write_char(')')?;
                }

                Ok(())
            }
            Node::Composed { terms } => {
                fn fmt_term(
                    f: &mut std::fmt::Formatter<'_>,
                    unit: NodeId,
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
                            unit.fetch().fmt(f)?;
                        }
                        (numer, denom) => {
                            unit.fetch().fmt(f)?;

                            if *denom == 1 {
                                f.write_str(&to_superscript(*numer as i64))?;
                            } else if denom != numer {
                                write!(f, "^{:.3}", exp.to_f64().unwrap())?;
                            }
                        }
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
