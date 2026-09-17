/* --------------------------------- TRAITS --------------------------------- */

use std::{collections::HashMap, fmt::Debug};

use faer::Mat;
use itertools::Itertools;
use num::{Complex, complex::Complex64};
use ordered_float::Pow;
use ripopt::{NlpProblem, SolveStatus, solve};

use crate::{
    core::value::Value,
    diff::Differentiable,
    expr::{
        Binding, Expr,
        ops::{imag, real},
    },
    model::{
        Variable,
        eq::{Constraint, Equation},
    },
    simplify::{Simplify, SimplifyContext},
    symbol::{Realization, Symbol},
};

pub trait Solver {
    type Error: Debug;

    fn solve(
        &self,
        eqs: Vec<Equation>,
        guesses: &HashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error>;
}

/* --------------------------------- STRUCTS -------------------------------- */
