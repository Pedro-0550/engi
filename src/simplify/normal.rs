use std::{collections::HashMap, iter::once, mem::discriminant};

use itertools::Itertools;
use num::complex::ComplexFloat;

use super::separate_consts;
use crate::{
    core::value::Scalar,
    expr::{
        Expr, Node,
        ops::{Atan2, Binary, Log, Pow, Unary, Variadic},
    },
    symbol::Symbol,
    units::Quantity,
};

/* --------------------------------- TRAITS --------------------------------- */

/// Adds support for conversion into a standard form, without touching symbols or simplifying algebraic constructions
/// To be exact, normalize will only:
///  * Flatten nested variadics;
///  * Fold constants into a single term;
///  * And sort terms in a standard, deterministic order
pub trait Normalize {
    fn normalize(&self, recurse: bool) -> Expr;

    /// Returns the rank of this expression, not considering its children.
    /// In this context, rank defines the sorting order during normalization.
    /// Dont confuse this with the rank operation, which returns the rank of a tensor.
    fn rank(&self) -> usize;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Normalize for Variadic {
    fn normalize(&self, recurse: bool) -> Expr {
        let normalized = self
            .operands()
            .iter()
            .map(|x| if recurse { x.normalize(recurse) } else { x.clone() });

        let flattened = normalized.flat_map(|expr| match expr.node() {
            Node::Variadic(op) if discriminant(op) == discriminant(self) => {
                op.clone().into_operands()
            }
            _ => vec![expr],
        });

        let (consts, exprs) = separate_consts(flattened);

        let mut result = match self {
            Variadic::Add(_) => {
                let mut exprs = exprs.collect_vec();

                let folded_const = consts.fold(0.into(), |acc: Quantity, x| {
                    (acc.value() + x.value()) * x.unit()
                });

                if folded_const.value() != 0.0 || exprs.len() == 0 {
                    exprs.push(folded_const.into());
                }

                exprs
            }
            Variadic::Mul(_) => {
                let folded_const =
                    consts.fold(1.into(), |acc: Quantity, x| acc * x);

                if folded_const.value() == 0.0.into() {
                    return 0.0.into();
                }

                let mut exprs = exprs.collect_vec();

                if folded_const.value() != 1.0.into() || exprs.len() == 0 {
                    exprs.push(folded_const.into());
                }

                exprs
            }
        };

        result.sort_unstable();

        if result.len() <= 1 {
            result.pop().unwrap_or(0.into())
        } else {
            self.with_operands(result).into()
        }
    }

    fn rank(&self) -> usize {
        match self {
            Variadic::Mul(_) => 0,
            Variadic::Add(_) => 1,
        }
    }
}

impl Normalize for Unary {
    fn normalize(&self, recurse: bool) -> Expr {
        self.with_arg(self.arg().normalize(recurse)).into()
    }

    fn rank(&self) -> usize {
        match self {
            // Why does this start at one? We had a 0 variant but i removed it, and writing this comment definetly took
            // less time than shifting all the numbers.
            Unary::Sin(_) => 1,
            Unary::Cos(_) => 2,
            Unary::Tan(_) => 3,
            Unary::Asin(_) => 4,
            Unary::Acos(_) => 5,
            Unary::Atan(_) => 6,
            Unary::Sinh(_) => 7,
            Unary::Cosh(_) => 8,
            Unary::Tanh(_) => 9,
            Unary::Asinh(_) => 10,
            Unary::Acosh(_) => 11,
            Unary::Atanh(_) => 12,
            Unary::Transpose(_) => 13,
            Unary::Conj(_) => 14,
            Unary::Arg(_) => 15,
            Unary::Det(_) => 16,
            Unary::Norm(_) => 17,
            Unary::Real(_) => 18,
            Unary::Imag(_) => 19,
        }
    }
}

impl Normalize for Binary {
    fn normalize(&self, recurse: bool) -> Expr {
        self.with_args([
            self.args()[0].normalize(recurse),
            self.args()[1].normalize(recurse),
        ])
        .into()
    }

    fn rank(&self) -> usize {
        match self {
            Binary::Pow(..) => 0,
            Binary::Log(..) => 1,
            Binary::Atan2(..) => 2,
        }
    }
}

impl Normalize for Expr {
    fn normalize(&self, recurse: bool) -> Self {
        match &*self.node() {
            Node::Symbol(_) => self.clone(),
            Node::Const(_) => self.clone(),
            Node::Variadic(variadic) => variadic.normalize(recurse),
            Node::Unary(single) => single.normalize(recurse),
            Node::Binary(double) => double.normalize(recurse),
            Node::Matrix(_matrix) => todo!(),
        }
    }

    fn rank(&self) -> usize {
        match *self.node() {
            Node::Const(_) => 0,
            Node::Symbol(_) => 1,
            Node::Unary(_) => 2,
            Node::Binary(_) => 3,
            Node::Variadic(_) => 4,
            Node::Matrix(_) => 5,
        }
    }
}

impl Ord for Expr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank()).then_with(|| {
            match (self.node(), other.node()) {
                (Node::Symbol(lhs), Node::Symbol(rhs)) => lhs.cmp(&rhs),
                (Node::Const(lhs), Node::Const(rhs)) => {
                    let lhs = lhs.value();
                    let rhs = rhs.value();
                    lhs.norm()
                        .total_cmp(&rhs.norm())
                        .then_with(|| lhs.arg().total_cmp(&rhs.arg()))
                }
                (Node::Unary(lhs), Node::Unary(rhs)) => lhs
                    .rank()
                    .cmp(&rhs.rank())
                    .then_with(|| lhs.arg().cmp(&rhs.arg())),
                (Node::Binary(lhs), Node::Binary(rhs)) => lhs
                    .rank()
                    .cmp(&rhs.rank())
                    .then_with(|| lhs.args()[0].cmp(&rhs.args()[0]))
                    .then_with(|| lhs.args()[1].cmp(&rhs.args()[1])),
                (Node::Variadic(lhs), Node::Variadic(rhs)) => {
                    lhs.rank().cmp(&rhs.rank()).then_with(|| {
                        lhs.operands().iter().cmp(rhs.operands().iter())
                    })
                }
                (Node::Matrix(_lhs), Node::Matrix(_rhs)) => todo!(),
                _ => unreachable!(
                    "Only two nodes of the same variant can be Ordering::Equal",
                ),
            }
        })
    }
}

impl PartialOrd for Expr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name().cmp(&other.name()).then_with(|| self.0.0.cmp(&other.0.0))
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod test {
    use crate::{simplify::normal::Normalize, symbol::Symbol, units::Unit};

    #[test]
    fn normalization() {
        let a = Symbol::new("a");
        let b = Symbol::new("b");
        let c = Symbol::new("c");

        panic!(
            "{}, {}",
            (a * b * -c + 0).normalize(true),
            (-(1 * a * b * c)).normalize(true)
        );
    }
}
