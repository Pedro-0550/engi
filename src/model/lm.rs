/* --------------------------------- TRAITS --------------------------------- */

use std::fmt::Debug;

use ahash::AHashMap;
use itertools::Itertools;
use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Matrix, Owned, Vector};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

use crate::{
    expr::{Expr, domain::Realized, jit::CompiledExpr},
    math::{Value, complex::Complex, real::Real},
    model::{Solver, Variable},
    r,
    symbol::{Realization, Symbol},
};

/* --------------------------------- STRUCTS -------------------------------- */

pub struct LmSolver;
pub struct LmProblem {
    residuals: Vec<CompiledExpr>,
    jacobian: Vec<Vec<CompiledExpr>>,
    params: Vec<Real>,
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

        let scale_bindings = symbols
            .iter()
            .map(|s| {
                let primary = s.as_primary().unwrap();
                let scale = guesses
                    .get(&Variable(primary))
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

        let residuals: Vec<_> = residuals
            .into_par_iter()
            .map(|r| Realized::from_real(r).compile())
            .collect();

        let problem = LmProblem {
            params: vec![r!(1); residuals.len()],
            jacobian,
            residuals,
            symbols: symbols.clone(),
        };
        // Massive hack since F: Float
        let (p, _) =
            LevenbergMarquardt::new().with_ftol(1e-18).minimize(problem);

        // let objective = Realized::from_real(objective).compile();

        // let mut optimizer = Nlopt::new(
        //     self.algo,
        //     symbols.len(),
        //     |x, grad, _| {
        //         let bindings = symbols
        //             .iter()
        //             .enumerate()
        //             .map(|(i, s)| (*s, x[i]))
        //             .collect();

        //         if let Some(grad) = grad {
        //             for (i, g) in gradient.iter().enumerate() {
        //                 grad[i] =
        //                     g.eval_realized(&bindings).as_scalar().unwrap().re;
        //             }
        //         }

        //         let f =
        //             objective.eval_realized(&bindings).as_scalar().unwrap().re;

        //         f
        //     },
        //     Target::Minimize,
        //     (),
        // );

        // optimizer.set_ftol_rel(1e-18).unwrap();
        // optimizer.set_xtol_rel(1e-18).unwrap();
        // let mut x = vec![1.0; symbols.len()];

        // match optimizer.optimize(&mut x) {
        //     Ok((_, resid)) | Err((_, resid)) if resid > 1e-6 => {
        //         panic!("Could not converge")
        //     }
        //     _ => (),
        // }

        Ok(p.params
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let sym = symbols[i];
                let primary = sym.as_primary().unwrap();
                let scale = guesses
                    .get(&Variable(primary))
                    .and_then(|x| x.as_scalar().copied())
                    .unwrap_or(Complex::ONE);

                match sym.realization() {
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

// float hack
impl LeastSquaresProblem<f64, Dyn, Dyn> for LmProblem {
    type ResidualStorage = Owned<f64, Dyn>;
    type ParameterStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, Dyn>;

    fn set_params(&mut self, p: &DVector<f64>) {
        for (i, x) in p.iter().enumerate() {
            self.params[i] = (*x).into()
        }
    }

    fn params(&self) -> DVector<f64> {
        DVector::from_iterator(
            self.params.len(),
            self.params.iter().map(|x| (*x).into()),
        )
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        Some(DVector::from_iterator(
            self.residuals.len(),
            self.residuals.iter().map(|r| {
                r.eval_realized(
                    &self
                        .symbols
                        .iter()
                        .copied()
                        .zip(self.params.iter().copied())
                        .collect(),
                )
                .as_scalar_real()
                .unwrap()
                .into()
            }),
        ))
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
                            .zip(self.params.iter().copied())
                            .collect(),
                    )
                    .as_scalar_real()
                    .unwrap()
                    .into()
            },
        ))
    }
}
// impl Solver for SymbolicSolver {
//     type Error = ();

//     fn solve(
//         &self,
//         residuals: Vec<Expr>,
//         guesses: &AHashMap<Variable, Value>,
//     ) -> Result<Vec<(Variable, Value)>, Self::Error> {
//         assert_eq!(
//             residuals.len(),
//             1,
//             "SymbolicSolver can only solve 1-equation blocks",
//         );
//         let lhs = residuals.pop();
//         let rhs = Expr::from(0);
//     }
// }
