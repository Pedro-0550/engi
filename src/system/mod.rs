use core::panic;
use std::{collections::HashMap, rc::Rc};

use itertools::Either;
use num::complex::Complex;
use variadics_please::all_tuples;

use crate::{
    core::{
        graph::{BipartiteGraph, RightNode},
        scalar::{I, Scalar},
    },
    dimension::{Quantity, si::Hz},
    equation, equations,
    expr::{Expr, ops::sin},
    symbol::Symbol,
    symbols,
    system::{
        eq::{Equation, Inequation},
        var::Variable,
    },
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;
pub mod var;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Dependencies {
    type Solution;
}

macro_rules! impl_dependencies {
    ($($T:ident),*) => {
        impl<$($T: System),*> Dependencies for ($($T,)*) {
            type Solution = ($($T::Solution,)*);
        }
    };
}

all_tuples!(impl_dependencies, 0, 12, T);

pub trait HasVariables {
    fn variables(&self) -> Vec<Variable>;
}

pub trait HasSolution {
    type Solution;
}

pub trait System: HasVariables + HasSolution {
    type Dependencies: Dependencies = ();

    fn equations(
        &self,
        deps: <Self::Dependencies as Dependencies>::Solution,
    ) -> Vec<Equation>;
    fn constraints(
        &self,
        deps: <Self::Dependencies as Dependencies>::Solution,
    ) -> Vec<Inequation>;

    fn structure(
        &self,
        deps: <Self::Dependencies as Dependencies>::Solution,
    ) -> SystemStructure {
        // Equation <-> Variable
        let mut incidence = BipartiteGraph::new();

        let equations = self.equations(deps);
        let variables = self.variables();

        for _ in &variables {
            incidence.add_right();
        }

        for eq in &equations {
            let eq_node = incidence.add_left();

            for symbol in eq.symbols() {
                let var_node = RightNode::new(
                    variables
                        .iter()
                        .position(|x| x.symbol() == symbol)
                        .unwrap(),
                );

                incidence.add_edge(eq_node, var_node);
            }
        }

        SystemStructure { incidence, equations, variables }
    }
}

pub struct SystemStructure {
    incidence: BipartiteGraph,
    equations: Vec<Equation>,
    variables: Vec<Variable>,
}

/* --------------------------------- STRUCTS -------------------------------- */

pub enum Solvability {
    Symbolic,
    Numeric,
}

pub struct Solution {
    solved: HashMap<Variable, Scalar>,
    observed: HashMap<Variable, Scalar>,
    residuals: HashMap<Equation, f64>,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl SystemStructure {
    fn simplify(&mut self) {
        let matching = self.incidence.maximum_matching();
        if !matching.is_perfect() {
            panic!()
        }

        let mut dependency_graph = Vec::new();

        for (eq_node, var_node) in matching.edges() {
            let eq = &self.equations[eq_node.index()];
            let var = &self.variables[var_node.index()];
            for sym in eq.symbols() {
                if sym == var.symbol() {
                    continue;
                }

                let sym_idx = self
                    .variables
                    .iter()
                    .position(|x| x.symbol() == sym)
                    .unwrap();

                dependency_graph.push((sym_idx, var_node.index()));
            }
        }

        panic!("{:#?}", dependency_graph);
    }
}

/* -------------------------------------------------------------------------- */

/// This macro dosen't implement System itself, but rather its subtraits, HasVariables and HasSolution.
#[macro_export]
macro_rules! System {
    derive() (
        $vis:vis struct $name:ident {
            $($field:ident: Variable),+ $(,)?
        }
    ) => {
        #[automatically_derived]
        impl HasVariables for $name {
            fn variables(&self) -> Vec<Variable> {
                vec![$(self.$field),+]
            }
        }

        paste::paste! {
            $vis struct [<$name Solution>] {
                $($field: Quantity),+
            }

            #[automatically_derived]
            impl HasSolution for $name {
                type Solution = [<$name Solution>];
            }
        }

    };
}

#[derive(System)]
pub struct TestSystem {
    x: Variable,
    y: Variable,
}

impl Default for TestSystem {
    fn default() -> Self {
        Self {
            x: Variable::builder("x")
                .guess(1 + 10 * I)
                .unit(Hz)
                .desc("A variable")
                .build(),
            y: Variable::builder("y")
                .value(1 + 10 * I)
                .unit(Hz)
                .desc("A variable")
                .build(),
        }
    }
}

impl System for TestSystem {
    fn equations(&self, deps: Self::Dependencies) -> Vec<Equation> {
        equations! { self.x = self.y }
    }

    fn constraints(&self, deps: Self::Dependencies) -> Vec<Inequation> {
        equations! { self.x > 2.0 }
    }
}
