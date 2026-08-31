use num::complex::Complex64;

use crate::{
    expr::{
        Expr, Node,
        ops::{
            Atan2, Binary, Log, Pow, Unary, Variadic, cos, cosh, ln, pow, sin,
            sinh, sqrt,
        },
    },
    simplify::{Simplify, SimplifyContext, normal::Normalize},
    symbol::{Symbol, constants::e},
};

/* --------------------------------- MODULES -------------------------------- */

#[cfg(test)]
mod test;

/* --------------------------------- STRUCTS -------------------------------- */

/// Todo: Explain
pub struct Dual {
    pub z: Complex64,
    pub grad: Vec<Complex64>,
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Differentiable {
    fn diff(&self, symbol: Symbol) -> Expr;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Differentiable for Expr {
    fn diff(&self, s: Symbol) -> Expr {
        let mut ctx = SimplifyContext::new();
        match self.normalize(true).into_node() {
            Node::Const(_) => 0.into(),
            Node::Symbol(sym) => if sym == s { 1 } else { 0 }.into(),
            Node::Variadic(op) => op.diff(s),
            Node::Unary(op) => op.arg().diff(s) * op.diff(s),
            Node::Binary(op) => op.diff(s),
            _ => todo!(),
        }
        .simplify_inner(&mut ctx)
    }
}

impl Differentiable for Unary {
    fn diff(&self, s: Symbol) -> Expr {
        match self {
            Unary::Sin(u) => cos(u),
            Unary::Cos(u) => -sin(u),
            Unary::Tan(u) => 1 / pow(cos(u), 2),
            Unary::Asin(u) => 1 / sqrt(1 - pow(u, 2)),
            Unary::Acos(u) => -1 / sqrt(1 - pow(u, 2)),
            Unary::Atan(u) => 1 / (pow(u, 2) + 1),
            Unary::Sinh(u) => cosh(u),
            Unary::Cosh(u) => sinh(u),
            Unary::Tanh(u) => 1 / pow(cosh(u), 2),
            Unary::Asinh(u) => 1 / sqrt(pow(u, 2) + 1),
            Unary::Acosh(u) => 1 / sqrt(pow(u, 2) - 1),
            Unary::Atanh(u) => 1 / (1 - pow(u, 2)),
            Unary::Transpose(u) => Unary::Transpose(u.diff(s)).into(),
            Unary::Conj(_u) => todo!(),
            Unary::Arg(_u) => todo!(),
            Unary::Det(_u) => todo!(),
            Unary::Norm(_u) => todo!(),
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
                pow(base, exp)
                    * (base.diff(s) * exp / base + exp.diff(s) * ln(base))
            }
            Binary::Log(Log { base, arg }) => {
                if *base == e.into() {
                    arg.diff(s) / arg
                } else if base.diff(s) == 0.into() {
                    arg.diff(s) / (arg * ln(base))
                } else {
                    ((arg.diff(s) / arg) * ln(base)
                        - (base.diff(s) / base) * ln(arg))
                        / pow(ln(base), 2)
                }
            }
            Self::Atan2(Atan2 { a: _, b: _ }) => todo!(),
        }
    }
}
