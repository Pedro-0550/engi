/* --------------------------------- TRAITS --------------------------------- */

use std::fmt::Debug;

use ahash::AHashMap;
use itertools::Itertools;
use levenberg_marquardt::{
    LeastSquaresProblem, LevenbergMarquardt,
    TerminationReason::{self, NoImprovementPossible},
};
use nalgebra::{DMatrix, DVector, Dyn, Matrix, Normed, Owned, Vector, dvector};
use num::{Complex, Zero};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

use crate::{
    core::value::{EQ_REL_TOL, Value},
    expr::{Expr, domain::Realized, jit::CompiledExpr},
    model::{Solver, Variable},
    symbol::{Realization, Symbol},
};

/* --------------------------------- STRUCTS -------------------------------- */

pub struct LmSolver;
pub struct LmProblem {
    residuals: Vec<CompiledExpr>,
    jacobian: Vec<Vec<CompiledExpr>>,
    x: DVector<f64>,
    symbols: Vec<Symbol>,
}

// /// Solves 1-variable blocks symbolically, by manipulating equations algebraically, obtaining exact solutions
// pub struct SymbolicSolver;

/* ---------------------------------- IMPLS --------------------------------- */

// impl NloptSolver {
//     pub fn new(algo: Algorithm) -> Self {
//         Self { algo }
//     }
// }

impl Solver for LmSolver {
    type Error = levenberg_marquardt::TerminationReason;

    fn solve(
        &self,
        residuals: Vec<Expr>,
        // constraints: Vec<Constraint>,
        guesses: &AHashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error> {
        let realized_residuals = residuals
            .into_iter()
            .flat_map(|resid| resid.realize().unpack())
            .collect_vec();

        let mut symbols =
            realized_residuals.iter().flat_map(|eq| eq.symbols()).collect_vec();

        symbols.sort();
        symbols.dedup();

        fn scale_of(
            guesses: &AHashMap<Variable, Value>,
            primary: Symbol,
        ) -> Complex<f64> {
            guesses
                .get(&Variable(primary))
                .and_then(|x| x.as_scalar().copied())
                .filter(|x| !x.is_zero())
                .unwrap_or(Complex::ONE) // use this in BOTH the scale and unscale steps
        }

        let scale_bindings = symbols
            .iter()
            .map(|s| {
                let primary = s.as_primary().unwrap();
                let scale = scale_of(guesses, primary);

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

        let residuals = realized_residuals
            .into_iter()
            .map(|resid| {
                resid.substituted(&scale_bindings).unwrap_or(resid).simplified()
            })
            .collect_vec();

        let jacobian: Vec<_> = residuals
            .par_iter()
            .map(|r| {
                let rows: Vec<_> = symbols
                    .par_iter()
                    .map(|s| Realized::from_real(r.diff(*s)).compile())
                    .collect();
                rows
            })
            .collect();

        let compiled_residuals: Vec<_> = residuals
            .clone()
            .into_par_iter()
            .map(|r| Realized::from_real(r).compile())
            .collect();

        let problem = LmProblem {
            x: DVector::from_element(symbols.len(), 1.0),
            jacobian,
            residuals: compiled_residuals,
            symbols: symbols.clone(),
        };
        let (p, report) = LevenbergMarquardt::new()
            .with_ftol(1e-15)
            .with_xtol(1e-14)
            .with_gtol(1e-5)
            .with_patience(10000)
            .with_scale_diag(true)
            .minimize(problem);

        assert!(
            report.termination.was_successful(),
            "{:?}",
            &report.termination
        );

        assert!(
            report.objective_function.abs() < 1e-14,
            "{:.3e}",
            report.objective_function.abs()
        );

        let mut out: AHashMap<Variable, Complex<f64>> = AHashMap::new();
        for (i, v) in p.x.iter().enumerate() {
            let sym = symbols[i];
            let scale = scale_of(guesses, sym.as_primary().unwrap());
            match sym.realization() {
                Realization::Real(pri) => {
                    out.entry(Variable(pri)).or_default().re = scale.re * v
                }
                Realization::Imag(pri) => {
                    out.entry(Variable(pri)).or_default().im = scale.im * v
                }
                Realization::Primary => unreachable!(),
            }
        }
        Ok(out.into_iter().map(|(k, c)| (k, c.into())).collect())
    }
}

impl LmProblem {
    fn evaluate(&self) -> DVector<f64> {
        DVector::from_iterator(
            self.residuals.len(),
            self.residuals.iter().map(|r| {
                r.eval_realized(
                    &self
                        .symbols
                        .iter()
                        .copied()
                        .zip(self.x.iter().copied())
                        .collect(),
                )
                .as_scalar_real()
                .unwrap()
                .into()
            }),
        )
    }
}

impl LeastSquaresProblem<f64, Dyn, Dyn> for LmProblem {
    type ResidualStorage = Owned<f64, Dyn>;
    type ParameterStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, Dyn>;

    fn set_params(&mut self, p: &DVector<f64>) {
        for (i, x) in p.iter().enumerate() {
            self.x[i] = *x
        }
    }

    fn params(&self) -> DVector<f64> {
        DVector::from_iterator(self.x.len(), self.x.iter().copied())
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        let f = self.evaluate();
        // println!("x -> {}", self.params());
        // println!("f(x) -> {f}");

        Some(f)
    }

    fn jacobian(&self) -> Option<DMatrix<f64>> {
        Some(DMatrix::from_fn(
            self.residuals.len(),
            self.symbols.len(),
            |r, s| {
                self.jacobian[r][s]
                    .eval_realized(
                        &self
                            .symbols
                            .iter()
                            .copied()
                            .zip(self.x.iter().copied())
                            .collect(),
                    )
                    .as_scalar_real()
                    .unwrap()
                    .into()
            },
        ))
    }
}
