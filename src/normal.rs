use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
    iter::once,
    mem::discriminant,
};

use ahash::AHasher;
use itertools::Itertools;
use num::complex::ComplexFloat;

use super::separate_consts;
use crate::{
    core::value::ComplexExt,
    expr::{Expr, Node},
    symbol::Symbol,
    units::Quantity,
};

/* --------------------------------- TRAITS --------------------------------- */

/// Adds support for conversion into a standard form, without touching symbols or simplifying non-trivial algebraic constructions
/// To be exact, normalize will only:
///  * Flatten nested variadics;
///  * Fold constants into a single term;
///  * Apply additive and multiplicative identities;
///  * And sort terms in a standard, deterministic order
pub trait Normalize {
    fn normalize(&self, recurse: bool) -> Expr;

    /// Returns the rank of this expression, not considering its children.
    /// In this context, rank defines the sorting order during normalization.
    /// Dont confuse this with the rank operation, which returns the rank of a tensor.
    fn precedence(&self) -> usize;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    pub fn normalize(&self) -> Self {
        match self.node() {
            Node::Add(exprs)
            | Node::Mul(exprs)
            | Node::Min(exprs)
            | Node::Max(exprs) => {
                let flattened = self.iter_children().flat_map(|expr| {
                    let norm = expr.normalize();
                    if discriminant(&norm) == discriminant(self) {
                        norm.iter_children().cloned().collect()
                    } else {
                        vec![norm]
                    }
                });

                let (consts, mut exprs) = separate_consts(flattened);

                let mut result = match self.node() {
                    Node::Add(_) => {
                        let folded_const = consts.into_iter().fold(
                            0.into(),
                            |acc: Quantity, x| {
                                (acc.value() + x.value()) * x.unit()
                            },
                        );

                        if *folded_const.value() != 0.0 || exprs.len() == 0 {
                            exprs.push(folded_const.into());
                        }

                        exprs
                    }
                    Node::Mul(_) => {
                        let folded_const = consts
                            .into_iter()
                            .fold(1.into(), |acc: Quantity, x| acc * x);

                        if folded_const
                            .value()
                            .as_scalar()
                            .map(|x| x.eq_approx(0.0))
                            .is_some_and(|x| x)
                        {
                            return 0.0.into();
                        }

                        if *folded_const.value() != 1.0 || exprs.len() == 0 {
                            exprs.push(folded_const.into());
                        }

                        exprs
                    }
                    Node::Max(_) => {
                        let folded_const = consts
                            .into_iter()
                            .reduce(|a, b| {
                                a.value()
                                    .as_scalar_real()
                                    .expect("Max is only supported on scalar and purely real values.")
                                    .max(b.value().as_scalar_real()
                                    .expect("Max is only supported on scalar and purely real values.")) * a.unit()
                            })
                            .unwrap();

                        exprs.push(folded_const.into());

                        exprs
                    }
                    Node::Min(_) => {
                        let folded_const = consts
                            .into_iter()
                            .reduce(|a, b| a.value()
                                .as_scalar_real()
                                .expect("Min is only supported on scalar and purely real values.")
                                .min(b.value().as_scalar_real()
                                .expect("Min is only supported on scalar and purely real values.")) * a.unit())
                            .unwrap();

                        exprs.push(folded_const.into());

                        exprs
                    }
                    _ => unreachable!(),
                };

                result.sort_unstable();

                if result.len() <= 1 {
                    result.pop().unwrap_or(0.into())
                } else {
                    let exprs = exprs.into_boxed_slice();
                    match self.node() {
                        Node::Add(_) => Node::Add(exprs),
                        Node::Mul(_) => Node::Mul(exprs),
                        Node::Min(_) => Node::Min(exprs),
                        Node::Max(_) => Node::Max(exprs),
                        _ => unreachable!(),
                    }
                    .into()
                }
            }
            _ => self.map_children(|e| e.normalize()),
        }
    }
}

pub fn separate_consts(
    terms: impl IntoIterator<Item = Expr>,
) -> (Vec<Quantity>, Vec<Expr>) {
    let mut consts = Vec::new();
    let mut exprs = Vec::new();

    for expr in terms {
        match expr.node() {
            Node::Quantity(qty) => consts.push(qty.clone()),
            _ => exprs.push(expr),
        }
    }

    (consts, exprs)
}

// impl Normalize for Variadic {
//     fn normalize(&self, recurse: bool) -> Expr {
//         let normalized = self
//             .operands()
//             .iter()
//             .map(|x| if recurse { x.normalize(recurse) } else { x.clone() });

//         let flattened = normalized.flat_map(|expr| match expr.node() {
//             Node::Variadic(op) if discriminant(op) == discriminant(self) => {
//                 op.clone().into_operands()
//             }
//             _ => vec![expr],
//         });

//         let (consts, mut exprs) = separate_consts(flattened);

//         let mut result = match self {
//             Variadic::Add(_) => {
//                 let folded_const =
//                     consts.into_iter().fold(0.into(), |acc: Quantity, x| {
//                         (acc.value() + x.value()) * x.unit()
//                     });

//                 if *folded_const.value() != 0.0 || exprs.len() == 0 {
//                     exprs.push(folded_const.into());
//                 }

//                 exprs
//             }
//             Variadic::Mul(_) => {
//                 let folded_const = consts
//                     .into_iter()
//                     .fold(1.into(), |acc: Quantity, x| acc * x);

//                 if folded_const
//                     .value()
//                     .as_scalar()
//                     .map(|x| x.eq_approx(0.0))
//                     .is_some_and(|x| x)
//                 {
//                     return 0.0.into();
//                 }

//                 if *folded_const.value() != 1.0 || exprs.len() == 0 {
//                     exprs.push(folded_const.into());
//                 }

//                 exprs
//             }
//         };

//         result.sort_unstable();

//         if result.len() <= 1 {
//             result.pop().unwrap_or(0.into())
//         } else {
//             self.with_operands(result).into()
//         }
//     }

//     fn precedence(&self) -> usize {
//         match self {
//             Variadic::Mul(_) => 0,
//             Variadic::Add(_) => 1,
//         }
//     }
// }

// impl Normalize for Unary {
//     fn normalize(&self, recurse: bool) -> Expr {
//         self.with_arg(self.arg().normalize(recurse)).into()
//     }

//     fn precedence(&self) -> usize {
//         match self {
//             // Why does this start at one? We had a 0 variant but i removed it, and writing this comment definetly took
//             // less time than shifting all the numbers.
//             Unary::Sin(_) => 1,
//             Unary::Cos(_) => 2,
//             Unary::Tan(_) => 3,
//             Unary::Asin(_) => 4,
//             Unary::Acos(_) => 5,
//             Unary::Atan(_) => 6,
//             Unary::Sinh(_) => 7,
//             Unary::Cosh(_) => 8,
//             Unary::Tanh(_) => 9,
//             Unary::Asinh(_) => 10,
//             Unary::Acosh(_) => 11,
//             Unary::Atanh(_) => 12,
//             Unary::Transpose(_) => 13,
//             Unary::Conj(_) => 14,
//             Unary::Arg(_) => 15,
//             Unary::Det(_) => 16,
//             Unary::Norm(_) => 17,
//             Unary::Real(_) => 18,
//             Unary::Imag(_) => 19,
//             Unary::Sign(_) => 20,
//         }
//     }
// }

// impl Normalize for Binary {
//     fn normalize(&self, recurse: bool) -> Expr {
//         self.with_args([
//             self.args()[0].normalize(recurse),
//             self.args()[1].normalize(recurse),
//         ])
//         .into()
//     }

//     fn precedence(&self) -> usize {
//         match self {
//             Binary::Pow(..) => 0,
//             Binary::Log(..) => 1,
//             Binary::Atan2(..) => 2,
//         }
//     }
// }

// impl Normalize for Expr {
//     fn normalize(&self, recurse: bool) -> Self {
//         match &*self.node() {
//             Node::Symbol(_) => self.clone(),
//             Node::Quantity(_) => self.clone(),
//             Node::Constant(_) => self.clone(),
//             Node::Variadic(variadic) => variadic.normalize(recurse),
//             Node::Unary(single) => single.normalize(recurse),
//             Node::Binary(double) => double.normalize(recurse),
//             Node::Matrix(_matrix) => todo!(),
//         }
//     }

//     fn precedence(&self) -> usize {
//         match *self.node() {
//             Node::Quantity(_) => 0,
//             Node::Symbol(_) => 1,
//             Node::Constant(_) => 2,
//             Node::Unary(_) => 3,
//             Node::Binary(_) => 4,
//             Node::Variadic(_) => 5,
//             Node::Matrix(_) => 6,
//         }
//     }
// }

#[cfg(test)]
mod test {
    use crate::{normal::Normalize, symbol::Symbol, units::Unit};

    #[test]
    fn normalization() {
        let a = Symbol::new("a");
        let b = Symbol::new("b");
        let c = Symbol::new("c");

        assert_eq!(
            (a * b * -c + 0).normalize(),
            (-(1 * a * b * c)).normalize()
        );
    }
}
