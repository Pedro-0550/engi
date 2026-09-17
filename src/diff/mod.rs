use num::complex::Complex64;
use ordered_float::Pow as _;

use crate::{
    expr::{
        Domain, Expr, Node,
        ops::{
            Atan2, Binary, Log, Pow, Unary, Variadic, cos, cosh, ln, sin, sinh,
            sqrt,
        },
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
        match self.normalize(true).into_node() {
            Node::Quantity(_) => 0.into(),
            Node::Constant(_) => 0.into(),
            Node::Symbol(sym) => if sym == s { 1 } else { 0 }.into(),
            Node::Variadic(op) => op.diff(s),
            Node::Unary(op) => op.arg().diff(s) * op.diff(s),
            Node::Binary(op) => op.diff(s),
            _ => todo!(),
        }
        .normalize(true)
    }
}

impl Differentiable for Unary {
    fn diff(&self, s: Symbol) -> Expr {
        match self {
            Unary::Sin(u) => cos(u),
            Unary::Cos(u) => -sin(u),
            Unary::Tan(u) => 1 / cos(u).pow(2),
            Unary::Asin(u) => 1 / sqrt(1 - u.pow(2)),
            Unary::Acos(u) => -1 / sqrt(1 - u.pow(2)),
            Unary::Atan(u) => 1 / (u.pow(2) + 1),
            Unary::Sinh(u) => cosh(u),
            Unary::Cosh(u) => sinh(u),
            Unary::Tanh(u) => 1 / cosh(u).pow(2),
            Unary::Asinh(u) => 1 / sqrt(u.pow(2) + 1),
            Unary::Acosh(u) => 1 / sqrt(u.pow(2) - 1),
            Unary::Atanh(u) => 1 / (1 - u.pow(2)),
            Unary::Transpose(u) => Unary::Transpose(u.diff(s)).into(),
            Unary::Conj(u) => match u.domain() {
                Domain::Real => u.diff(s),
                Domain::Imag => -u.diff(s),
                Domain::Complex => todo!("We're still developing the funny"),
            },
            Unary::Arg(_u) => todo!(),
            Unary::Det(_u) => todo!(),
            Unary::Norm(_u) => todo!(),
            Unary::Real(u) => match u.domain() {
                Domain::Real => u.diff(s),
                Domain::Imag => 0.into(),
                Domain::Complex => todo!("We're still developing the funny"),
            },
            Unary::Imag(u) => match u.domain() {
                Domain::Real => 0.into(),
                Domain::Imag => u.diff(s),
                Domain::Complex => todo!("We're still developing the funny"),
            },
        }
    }
}

impl Differentiable for Variadic {
    fn diff(&self, s: Symbol) -> Expr {
        match self {
            Variadic::Add(terms) => {
                Variadic::Add(terms.iter().map(|expr| expr.diff(s)).collect())
                    .into()
            }
            Variadic::Mul(terms) => Variadic::Add(
                terms
                    .iter()
                    .enumerate()
                    .map(|(i, expr)| {
                        let mut factors = Vec::with_capacity(terms.len());
                        factors.push(expr.diff(s));
                        factors.extend(terms.iter().enumerate().filter_map(
                            |(j, x)| (i != j).then_some(x.clone()),
                        ));
                        Variadic::Mul(factors).into()
                    })
                    .collect(),
            )
            .into(),
        }
    }
}

impl Differentiable for Binary {
    fn diff(&self, s: Symbol) -> Expr {
        match self {
            Binary::Pow(Pow { base, exp }) => {
                if exp.diff(s) == 0 {
                    exp * base.pow(exp - 1) * base.diff(s)
                } else {
                    base.pow(exp)
                        * (base.diff(s) * exp / base + exp.diff(s) * ln(base))
                }
            }
            Binary::Log(Log { base, arg }) => {
                if *base == e {
                    arg.diff(s) / arg
                } else if base.diff(s) == 0 {
                    arg.diff(s) / (arg * ln(base))
                } else {
                    ((arg.diff(s) / arg) * ln(base)
                        - (base.diff(s) / base) * ln(arg))
                        / ln(base).pow(2)
                }
            }
            Self::Atan2(Atan2 { a, b }) => {
                (b * a.diff(s) - a * b.diff(s)) / (a.pow(2) + b.pow(2))
            }
        }
    }
}
