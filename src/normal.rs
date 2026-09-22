use std::{
    cmp::Ordering,
    collections::HashMap,
    hash::{BuildHasher, Hash},
    iter::once,
    mem::discriminant,
};

use ahash::AHasher;
use itertools::Itertools;
use num::complex::ComplexFloat;

use crate::{
    core::value::ComplexExt,
    expr::{Expr, Node},
    symbol::Symbol,
    units::Quantity,
};

/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    /// Converts into a standard form, without touching symbols or simplifying non-trivial algebraic constructions
    /// To be exact, normalize will only:
    ///  * Flatten nested variadics;
    ///  * Fold constants into a single term;
    ///  * Apply additive and multiplicative identities;
    ///  * And sort terms in a standard, deterministic order
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

                result.sort_unstable_by_key(|e| e.key());

                if result.len() <= 1 {
                    result.pop().unwrap_or(0.into())
                } else {
                    let result = result.into_boxed_slice();
                    match self.node() {
                        Node::Add(_) => Node::Add(result),
                        Node::Mul(_) => Node::Mul(result),
                        Node::Min(_) => Node::Min(result),
                        Node::Max(_) => Node::Max(result),
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

#[cfg(test)]
mod test {
    use crate::{symbol::Symbol, units::Unit};

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
