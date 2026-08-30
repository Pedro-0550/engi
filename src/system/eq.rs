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
    pub fn new(lhs: Expr, rhs: Expr, ineq: Inequality) -> Self {
        Self { lhs, rhs, ineq }
    }
}

impl Equation {
    pub fn new(lhs: Expr, rhs: Expr) -> Self {
        Self { lhs, rhs }
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
