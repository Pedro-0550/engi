/* --------------------------------- TRAITS --------------------------------- */

use std::{collections::HashMap, fmt::Debug};

use faer::Mat;
use itertools::Itertools;
use nlopt::{Algorithm, Nlopt, Target};
use num::{Complex, complex::Complex64};
use ordered_float::Pow;

use crate::{
    core::value::{EQ_ABS_TOL, Value},
    diff::Differentiable,
    expr::Expr,
    model::{
        Variable,
        eq::{Constraint, Equation},
    },
    simplify::{Simplify, SimplifyContext, normal::Normalize},
    symbol::{Realization, Symbol},
};

pub trait Solver {
    type Error: Debug;

    fn solve(
        &self,
        residuals: Vec<Expr>,
        guesses: &HashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error>;
}

/* --------------------------------- STRUCTS -------------------------------- */

pub struct NloptSolver {
    algo: Algorithm,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl NloptSolver {
    pub fn new(algo: Algorithm) -> Self {
        Self { algo }
    }
}

impl Solver for NloptSolver {
    type Error = nlopt::FailState;

    fn solve(
        &self,
        residuals: Vec<Expr>,
        // constraints: Vec<Constraint>,
        guesses: &HashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error> {
        let mut symbols = residuals
            .iter()
            .flat_map(|eq| eq.symbols())
            .flat_map(|s| [s.real().unwrap(), s.imag().unwrap()])
            .collect_vec();

        symbols.sort();
        symbols.dedup();

        let scale_bindings = symbols
            .iter()
            .map(|s| {
                let scale = guesses
                    .get(&Variable(*s))
                    .and_then(|x| x.as_scalar().copied())
                    .unwrap_or(Complex::ONE);
                (
                    *s,
                    match s.realization() {
                        Realization::Primary => unreachable!(),
                        Realization::Real(_) => scale.re * s,
                        Realization::Imag(_) => scale.im * s,
                    },
                )
            })
            .collect();

        let residuals = residuals
            .into_iter()
            .flat_map(|eq| eq.realize().into())
            .map(|resid: Expr| {
                resid.substitute(&scale_bindings).normalize(true)
            })
            .collect_vec();

        let objective = residuals
            .iter()
            .fold(Expr::from(0.0), |acc, resid| acc + resid.pow(2))
            .normalize(true);

        let gradient =
            symbols.iter().map(|s| objective.diff(*s).compile()).collect_vec();
        let objective = objective.compile();

        let mut optimizer = Nlopt::new(
            self.algo,
            symbols.len(),
            |x, grad, _| {
                let bindings = symbols
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (*s, x[i]))
                    .collect();

                if let Some(grad) = grad {
                    for (i, g) in gradient.iter().enumerate() {
                        grad[i] =
                            g.eval_realized(&bindings).as_scalar().unwrap().re;
                    }
                }

                let f =
                    objective.eval_realized(&bindings).as_scalar().unwrap().re;

                println!("iter f = {}", f);

                f
            },
            Target::Minimize,
            (),
        );

        optimizer.set_ftol_rel(1e-12).unwrap();
        optimizer.set_xtol_rel(1e-12).unwrap();

        let mut x = vec![1.0; symbols.len()];

        match optimizer.optimize(&mut x) {
            Ok((_, resid)) | Err((_, resid)) if resid > 1e-6 => {
                panic!("Could not converge")
            }
            _ => (),
        }

        Ok(x.into_iter()
            .enumerate()
            .map(|(i, v)| {
                let var = Variable(symbols[i]);
                let scale = guesses
                    .get(&var)
                    .and_then(|x| x.as_scalar().copied())
                    .unwrap_or(Complex::ONE);

                match var.0.realization() {
                    Realization::Real(pri) => {
                        (Variable(pri), (scale.re * v).into())
                    }
                    Realization::Imag(pri) => {
                        (Variable(pri), (scale.im * v).into())
                    }
                    Realization::Primary => unreachable!(),
                }
            })
            .collect_vec())
    }
}
