use crate::{expr::Expr, symbol::Symbol};

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Clone, PartialEq)]
pub struct Equation {
    lhs: Expr,
    rhs: Expr,
}

#[derive(Clone, PartialEq)]
pub enum Inequality {
    Greater,
    GreaterOrEq,
    Less,
    LessOrEq,
}

#[derive(Clone, PartialEq)]
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

    pub fn symbols(&self) -> Vec<Symbol> {
        let mut result = Vec::new();

        result.extend(self.lhs.symbols());
        result.extend(self.rhs.symbols());

        result.sort_unstable();
        result.dedup();

        result
    }
}
