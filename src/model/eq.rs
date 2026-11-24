use std::{fmt::Display, slice::Concat};

use crate::{
    expr::Expr,
    model::state::{Stateable, Stateful, States},
    symbol::Symbol,
    unit::Dimensioned,
};

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Clone, Debug, PartialEq)]
pub struct Equation {
    lhs: Expr,
    rhs: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Inequality {
    Greater,
    GreaterOrEq,
    Less,
    LessOrEq,
}

#[derive(Clone, Debug, PartialEq)]
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
        let lhs = lhs.into();
        let rhs = rhs.into();
        let lhs_unit = lhs.unit().unwrap();
        let rhs_unit = rhs.unit().unwrap();

        if lhs_unit != rhs_unit {
            panic!(
                "Unit mismatch: lhs = {lhs_unit}, rhs = {rhs_unit} in {lhs} = {rhs}",
            )
        }

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

/* -------------------------------------------------------------------------- */

// boilerplate?? in MY rust library??

pub struct Equations(pub Vec<Equation>);

impl<A, B> From<(A, B)> for Equations
where
    A: Into<Expr>,
    B: Into<Expr>,
{
    fn from((lhs, rhs): (A, B)) -> Self {
        Equations(vec![Equation::new(lhs, rhs)])
    }
}

impl<A, B, S> From<(Stateful<A, S>, B)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Clone,
    S: States,
{
    fn from((lhs_states, rhs): (Stateful<A, S>, B)) -> Self {
        Equations(
            lhs_states
                .into_iter()
                .map(|lhs| Equation::new(lhs, rhs.clone()))
                .collect(),
        )
    }
}

impl<A, B, S> From<(A, Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Stateable,
    S: States,
{
    fn from((lhs, rhs_states): (A, Stateful<B, S>)) -> Self {
        Equations(
            rhs_states
                .into_iter()
                .map(|rhs| Equation::new(lhs.clone(), rhs))
                .collect(),
        )
    }
}

impl<A, B, S> From<(Stateful<A, S>, Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
{
    fn from(
        (lhs_states, rhs_states): (Stateful<A, S>, Stateful<B, S>),
    ) -> Self {
        Equations(
            rhs_states
                .into_iter()
                .zip(lhs_states.into_iter())
                .map(|(lhs, rhs)| Equation::new(lhs, rhs))
                .collect(),
        )
    }
}

impl<A, B, S> From<(&Stateful<A, S>, B)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Clone,
    S: States,
    Stateful<A, S>: Clone,
{
    fn from((lhs_states, rhs): (&Stateful<A, S>, B)) -> Self {
        Self::from((lhs_states.clone(), rhs))
    }
}

impl<A, B, S> From<(A, &Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<B, S>: Clone,
{
    fn from((lhs, rhs_states): (A, &Stateful<B, S>)) -> Self {
        Self::from((lhs, rhs_states.clone()))
    }
}

impl<A, B, S> From<(&Stateful<A, S>, Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<A, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states): (&Stateful<A, S>, Stateful<B, S>),
    ) -> Self {
        Self::from((lhs_states.clone(), rhs_states))
    }
}

impl<A, B, S> From<(Stateful<A, S>, &Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<B, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states): (Stateful<A, S>, &Stateful<B, S>),
    ) -> Self {
        Self::from((lhs_states, rhs_states.clone()))
    }
}

impl<A, B, S> From<(&Stateful<A, S>, &Stateful<B, S>)> for Equations
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<A, S>: Clone,
    Stateful<B, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states): (&Stateful<A, S>, &Stateful<B, S>),
    ) -> Self {
        Self::from((lhs_states.clone(), rhs_states.clone()))
    }
}

/* -------------------------------------------------------------------------- */

pub struct Constraints(pub Vec<Constraint>);

impl<A, B> From<(A, B, Inequality)> for Constraints
where
    A: Into<Expr>,
    B: Into<Expr>,
{
    fn from((lhs, rhs, ineq): (A, B, Inequality)) -> Self {
        Constraints(vec![Constraint::new(lhs, rhs, ineq)])
    }
}

impl<A, B, S> From<(Stateful<A, S>, B, Inequality)> for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Clone,
    S: States,
{
    fn from((lhs_states, rhs, ineq): (Stateful<A, S>, B, Inequality)) -> Self {
        Constraints(
            lhs_states
                .into_iter()
                .map(|lhs| Constraint::new(lhs, rhs.clone(), ineq.clone()))
                .collect(),
        )
    }
}

impl<A, B, S> From<(A, Stateful<B, S>, Inequality)> for Constraints
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Stateable,
    S: States,
{
    fn from((lhs, rhs_states, ineq): (A, Stateful<B, S>, Inequality)) -> Self {
        Constraints(
            rhs_states
                .into_iter()
                .map(|rhs| Constraint::new(lhs.clone(), rhs, ineq.clone()))
                .collect(),
        )
    }
}

impl<A, B, S> From<(Stateful<A, S>, Stateful<B, S>, Inequality)> for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
{
    fn from(
        (lhs_states, rhs_states, ineq): (
            Stateful<A, S>,
            Stateful<B, S>,
            Inequality,
        ),
    ) -> Self {
        Constraints(
            rhs_states
                .into_iter()
                .zip(lhs_states.into_iter())
                .map(|(lhs, rhs)| Constraint::new(lhs, rhs, ineq.clone()))
                .collect(),
        )
    }
}

impl<A, B, S> From<(&Stateful<A, S>, B, Inequality)> for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Clone,
    S: States,
    Stateful<A, S>: Clone,
{
    fn from((lhs_states, rhs, ineq): (&Stateful<A, S>, B, Inequality)) -> Self {
        Self::from((lhs_states.clone(), rhs, ineq))
    }
}

impl<A, B, S> From<(A, &Stateful<B, S>, Inequality)> for Constraints
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<B, S>: Clone,
{
    fn from((lhs, rhs_states, ineq): (A, &Stateful<B, S>, Inequality)) -> Self {
        Self::from((lhs, rhs_states.clone(), ineq))
    }
}

impl<A, B, S> From<(&Stateful<A, S>, Stateful<B, S>, Inequality)>
    for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<A, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states, ineq): (
            &Stateful<A, S>,
            Stateful<B, S>,
            Inequality,
        ),
    ) -> Self {
        Self::from((lhs_states.clone(), rhs_states, ineq))
    }
}

impl<A, B, S> From<(Stateful<A, S>, &Stateful<B, S>, Inequality)>
    for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<B, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states, ineq): (
            Stateful<A, S>,
            &Stateful<B, S>,
            Inequality,
        ),
    ) -> Self {
        Self::from((lhs_states, rhs_states.clone(), ineq))
    }
}

impl<A, B, S> From<(&Stateful<A, S>, &Stateful<B, S>, Inequality)>
    for Constraints
where
    A: Into<Expr> + Stateable,
    B: Into<Expr> + Stateable,
    S: States,
    Stateful<A, S>: Clone,
    Stateful<B, S>: Clone,
{
    fn from(
        (lhs_states, rhs_states, ineq): (
            &Stateful<A, S>,
            &Stateful<B, S>,
            Inequality,
        ),
    ) -> Self {
        Self::from((lhs_states.clone(), rhs_states.clone(), ineq))
    }
}
