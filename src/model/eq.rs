use std::fmt::Display;

use engi_macros::relation;

use crate as engi;
use crate::{
    expr::{
        Expr,
        ops::{imag, real},
    },
    symbol::Symbol,
};

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Clone, PartialEq, Debug, Hash, Eq)]
pub struct Equation {
    lhs: Expr,
    rhs: Expr,
}

#[derive(Clone, PartialEq, Debug, Hash, Eq)]
pub enum Inequality {
    Greater,
    GreaterOrEq,
    Less,
    LessOrEq,
}

#[derive(Clone, PartialEq, Debug, Hash, Eq)]
pub struct Constraint {
    lhs: Expr,
    rhs: Expr,
    ineq: Inequality,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Constraint {
    pub fn new(
        lhs: impl Into<Expr>,
        rhs: impl Into<Expr>,
        ineq: Inequality,
    ) -> Self {
        Self { lhs: lhs.into(), rhs: rhs.into(), ineq }
    }
}

impl Equation {
    pub fn new(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Self {
        Self { lhs: lhs.into(), rhs: rhs.into() }
    }

    pub fn residual(&self) -> Expr {
        self.rhs.clone() - self.lhs.clone()
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        let mut result = Vec::new();

        result.extend(self.lhs.symbols());
        result.extend(self.rhs.symbols());

        result.sort_unstable();
        result.dedup();

        result
    }

    pub fn lhs(&self) -> &Expr {
        &self.lhs
    }

    pub fn rhs(&self) -> &Expr {
        &self.rhs
    }
}

impl Display for Equation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.lhs.fmt(f)?;

        f.write_str(" = ")?;

        self.rhs.fmt(f)
    }
}
