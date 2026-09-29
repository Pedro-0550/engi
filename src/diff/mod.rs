use std::collections::HashMap;

use num::{Zero, complex::Complex64};

use crate::{
    core::value::{EQ_ABS_TOL, Set},
    expr::{
        self, Expr, Node, NodeId, cos, cosh,
        domain::{Domain, Numeric},
        ln, sin, sinh, sqrt,
        tree::{Branch, Leaf, Node},
    },
    symbol::{Symbol, constants::e},
};

/* --------------------------------- MODULES -------------------------------- */

#[cfg(test)]
mod test;

/* --------------------------------- TRAITS --------------------------------- */

// enum Derivative {
//     Conventional { dz: Expr },
//     Wirtinger { dz: Expr, dz_conj: Expr },
// }

pub trait Differentiable {
    fn diff(&self, symbol: Symbol) -> Expr;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Differentiable for Expr {
    fn diff(&self, s: Symbol) -> Expr {
        let mut dx = Expr::new();

        let (u, du) = self.fold_dfs(|id, node| {
            (
                id,
                match node {
                    Node::Leaf(leaf) => match leaf {
                        Leaf::Symbol(symbol) => {
                            if s == *symbol {
                                dx.one()
                            } else {
                                dx.zero()
                            }
                        }
                        Leaf::Constant(constant) => dx.zero(),
                        Leaf::Quantity(quantity) => dx.zero(),
                    },
                    Node::Branch(branch) => match branch {
                        Branch::Add([(a, da), (b, db)]) => dx.add(*da, *db),
                        Branch::Mul([(a, da), (b, db)]) => {
                            let a = dx.import(self, *a);
                            let b = dx.import(self, *b);

                            let x = dx.mul(a, *db);
                            let y = dx.mul(*da, b);
                            dx.add(x, y)
                        }
                        Branch::Min(_) => todo!(),
                        Branch::Max(_) => todo!(),
                        Branch::Sin(_) => todo!(),
                        Branch::Cos(_) => todo!(),
                        Branch::Tan(_) => todo!(),
                        Branch::Asin(_) => todo!(),
                        Branch::Acos(_) => todo!(),
                        Branch::Atan(_) => todo!(),
                        Branch::Sinh(_) => todo!(),
                        Branch::Cosh(_) => todo!(),
                        Branch::Tanh(_) => todo!(),
                        Branch::Asinh(_) => todo!(),
                        Branch::Acosh(_) => todo!(),
                        Branch::Atanh(_) => todo!(),
                        Branch::Arg(_) => todo!(),
                        Branch::Conj(_) => todo!(),
                        Branch::Norm(_) => todo!(),
                        Branch::Sign(_) => todo!(),
                        Branch::Real(d) => todo!(),
                        Branch::Imag(d) => todo!(),
                        Branch::Pow { base, exp } => todo!(),
                        Branch::Log { base, arg } => todo!(),
                        Branch::Atan2 { a, b } => todo!(),
                        Branch::Matrix(matrix) => todo!(),
                        Branch::Transpose(_) => todo!(),
                        Branch::Det(_) => todo!(),
                        Branch::Rank(_) => todo!(),
                        Branch::Trace(_) => todo!(),
                        Branch::Conditional { cond, pass, fail } => todo!(),
                    },
                },
            )
        });

        dx.set_root(du);

        dx

        // match self.node() {
        //     Node::Quantity(_) => 0.into(),
        //     Node::Constant(_) => 0.into(),
        //     Node::Symbol(sym) => if *sym == s { 1 } else { 0 }.into(),
        //     Node::Sin(box u) => u.diff(s) * cos(u),
        //     Node::Cos(box u) => u.diff(s) * -sin(u),
        //     Node::Tan(box u) => u.diff(s) / cos(u).pow(2),
        //     Node::Asin(box u) => u.diff(s) / sqrt(1 - u.pow(2)),
        //     Node::Acos(box u) => -u.diff(s) / sqrt(1 - u.pow(2)),
        //     Node::Atan(box u) => u.diff(s) / (u.pow(2) + 1),
        //     Node::Sinh(box u) => u.diff(s) * cosh(u),
        //     Node::Cosh(box u) => u.diff(s) * sinh(u),
        //     Node::Tanh(box u) => u.diff(s) / cosh(u).pow(2),
        //     Node::Asinh(box u) => u.diff(s) / sqrt(u.pow(2) + 1),
        //     Node::Acosh(box u) => u.diff(s) / sqrt(u.pow(2) - 1),
        //     Node::Atanh(box u) => u.diff(s) / (1 - u.pow(2)),
        //     Node::Transpose(u) => Node::Transpose(Box::new(u.diff(s))).into(),
        //     Node::Conj(u) => match u.domain().numeric() {
        //         Numeric::Real => u.diff(s),
        //         Numeric::Imag => -u.diff(s),
        //         Numeric::Complex => todo!("We're still developing the funny"),
        //     },
        //     Node::Arg(_u) => todo!(),
        //     Node::Det(_u) => todo!(),
        //     Node::Norm(_u) => todo!(),
        //     Node::Real(u) => match u.domain().numeric() {
        //         Numeric::Real => u.diff(s),
        //         Numeric::Imag => 0.into(),
        //         Numeric::Complex => todo!("We're still developing the funny"),
        //     },
        //     Node::Imag(u) => match u.domain().numeric() {
        //         Numeric::Real => 0.into(),
        //         Numeric::Imag => u.diff(s),
        //         Numeric::Complex => todo!("We're still developing the funny"),
        //     },
        //     Node::Sign(_) => 0.0.into(),
        //     Node::Add(terms) => {
        //         Node::Add(terms.iter().map(|expr| expr.diff(s)).collect())
        //             .into()
        //     }
        //     Node::Mul(terms) => Node::Add(
        //         terms
        //             .iter()
        //             .enumerate()
        //             .map(|(i, expr)| {
        //                 let mut factors = Vec::with_capacity(terms.len());
        //                 factors.push(expr.diff(s));
        //                 factors.extend(terms.iter().enumerate().filter_map(
        //                     |(j, x)| (i != j).then_some(x.clone()),
        //                 ));
        //                 Node::Mul(factors.into_boxed_slice()).into()
        //             })
        //             .collect(),
        //     )
        //     .into(),
        //     Node::Pow { box base, box exp } => {
        //         if exp.diff(s) == 0 {
        //             exp * base.pow(exp - 1) * base.diff(s)
        //         } else {
        //             base.pow(exp)
        //                 * (base.diff(s) * exp / base + exp.diff(s) * ln(base))
        //         }
        //     }
        //     Node::Log { box base, box arg } => {
        //         if base == e {
        //             arg.diff(s) / arg
        //         } else if base.diff(s) == 0 {
        //             arg.diff(s) / (arg * ln(base))
        //         } else {
        //             ((arg.diff(s) / arg) * ln(base)
        //                 - (base.diff(s) / base) * ln(arg))
        //                 / ln(base).pow(2)
        //         }
        //     }
        //     Node::Atan2 { box a, box b } => {
        //         (b * a.diff(s) - a * b.diff(s)) / (a.pow(2) + b.pow(2))
        //     }
        // }
        // .normalize()
    }
}
