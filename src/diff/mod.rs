use num::complex::Complex64;
use ordered_float::Pow as _;

use crate::{
    core::value::Set,
    expr::{
        self, Expr, Node, cos, cosh,
        domain::{Domain, Numeric},
        ln, sin, sinh, sqrt,
    },
    simplify::{Simplify, SimplifyContext, normal::Normalize},
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
        let mut ctx = SimplifyContext::new();
        match self.node() {
            Node::Quantity(_) => 0.into(),
            Node::Constant(_) => 0.into(),
            Node::Symbol(sym) => if *sym == s { 1 } else { 0 }.into(),
            Node::Sin(u) => u.diff(s) * cos(u),
            Node::Cos(u) => u.diff(s) * -sin(u),
            Node::Tan(u) => u.diff(s) / cos(u).pow(2),
            Node::Asin(u) => u.diff(s) / sqrt(1 - u.pow(2)),
            Node::Acos(u) => -u.diff(s) / sqrt(1 - u.pow(2)),
            Node::Atan(u) => u.diff(s) / (u.pow(2) + 1),
            Node::Sinh(u) => u.diff(s) * cosh(u),
            Node::Cosh(u) => u.diff(s) * sinh(u),
            Node::Tanh(u) => u.diff(s) / cosh(u).pow(2),
            Node::Asinh(u) => u.diff(s) / sqrt(u.pow(2) + 1),
            Node::Acosh(u) => u.diff(s) / sqrt(u.pow(2) - 1),
            Node::Atanh(u) => u.diff(s) / (1 - u.pow(2)),
            Node::Transpose(u) => Node::Transpose(Box::new(u.diff(s))).into(),
            Node::Conj(u) => match u.domain().numeric() {
                Numeric::Real => u.diff(s),
                Numeric::Imag => -u.diff(s),
                Numeric::Complex => todo!("We're still developing the funny"),
            },
            Node::Arg(_u) => todo!(),
            Node::Det(_u) => todo!(),
            Node::Norm(_u) => todo!(),
            Node::Real(u) => match u.domain().numeric() {
                Numeric::Real => u.diff(s),
                Numeric::Imag => 0.into(),
                Numeric::Complex => todo!("We're still developing the funny"),
            },
            Node::Imag(u) => match u.domain().numeric() {
                Numeric::Real => 0.into(),
                Numeric::Imag => u.diff(s),
                Numeric::Complex => todo!("We're still developing the funny"),
            },
            Node::Sign(_) => 0.0.into(),
            Node::Add(terms) => {
                Node::Add(terms.iter().map(|expr| expr.diff(s)).collect())
                    .into()
            }
            Node::Mul(terms) => Node::Add(
                terms
                    .iter()
                    .enumerate()
                    .map(|(i, expr)| {
                        let mut factors = Vec::with_capacity(terms.len());
                        factors.push(expr.diff(s));
                        factors.extend(terms.iter().enumerate().filter_map(
                            |(j, x)| (i != j).then_some(x.clone()),
                        ));
                        Node::Mul(factors.into_boxed_slice()).into()
                    })
                    .collect(),
            )
            .into(),
            Node::Pow { box base, box exp } => {
                if exp.diff(s) == 0 {
                    exp * base.pow(exp - 1) * base.diff(s)
                } else {
                    base.pow(exp)
                        * (base.diff(s) * exp / base + exp.diff(s) * ln(base))
                }
            }
            Node::Log { box base, box arg } => {
                if base == e {
                    arg.diff(s) / arg
                } else if base.diff(s) == 0 {
                    arg.diff(s) / (arg * ln(base))
                } else {
                    ((arg.diff(s) / arg) * ln(base)
                        - (base.diff(s) / base) * ln(arg))
                        / ln(base).pow(2)
                }
            }
            Node::Atan2 { box a, box b } => {
                (b * a.diff(s) - a * b.diff(s)) / (a.pow(2) + b.pow(2))
            }
        }
        .normalize(true)
    }
}
