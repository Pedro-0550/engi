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
    expr::{
        Binding, Expr,
        ops::{imag, real},
    },
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
        eqs: Vec<Equation>,
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
        eqs: Vec<Equation>,
        // constraints: Vec<Constraint>,
        guesses: &HashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error> {
        let mut symbols = eqs
            .iter()
            .flat_map(|eq| eq.symbols())
            .flat_map(|s| [s.real(), s.imag()])
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
                Binding::new(
                    *s,
                    match s.realization() {
                        Realization::Primary => unreachable!(),
                        Realization::Real(_) => scale.re * s,
                        Realization::Imag(_) => scale.im * s,
                    },
                )
            })
            .collect_vec();

        let residuals = eqs
            .into_iter()
            .flat_map(|eq| eq.residual().realize())
            .map(|resid| resid.substitute(&scale_bindings).normalize(true))
            .collect_vec();

        let objective = residuals
            .iter()
            .fold(Expr::from(0.0), |acc, resid| acc + resid.pow(2))
            .normalize(true);

        let gradient = symbols.iter().map(|s| objective.diff(*s)).collect_vec();

        let mut optimizer = Nlopt::new(
            self.algo,
            symbols.len(),
            |x, grad, _| {
                let bindings = symbols
                    .iter()
                    .enumerate()
                    .map(|(i, s)| Binding::new(*s, x[i].into()))
                    .collect_vec();

                if let Some(grad) = grad {
                    for (i, g) in gradient.iter().enumerate() {
                        grad[i] = g
                            .eval(&bindings)
                            .node()
                            .as_quantity()
                            .unwrap()
                            .value()
                            .as_scalar()
                            .unwrap()
                            .re;
                    }
                }

                let f = objective
                    .eval(&bindings)
                    .node()
                    .as_quantity()
                    .unwrap()
                    .value()
                    .as_scalar()
                    .unwrap()
                    .re;

                println!("iter f = {}", f);

                f
            },
            Target::Minimize,
            (),
        );

        optimizer.set_ftol_rel(1e-12).unwrap();
        optimizer.set_xtol_rel(1e-12).unwrap();

        let mut x = vec![1.0; symbols.len()];

        optimizer.optimize(&mut x).unwrap();

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
