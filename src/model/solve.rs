use std::fmt::Debug;

use ahash::AHashMap;
use tabled::Table;

use crate::{
    core::value::Value,
    expr::Expr,
    model::{Associated, Condition, Model, ModelBuilder, System, Variable},
    unit::Quantity,
};

pub mod lm;

pub trait Solver {
    type Error: Debug;

    fn solve(
        &self,
        residuals: Vec<Expr>,
        guesses: &AHashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error>;
}

/* -------------------------------------------------------------------------- */

pub trait InterfaceSolution {
    fn disassemble(assembled: AssembledInterfaceSolution) -> Self;
    fn collect_sections(
        &self,
        into: &mut Vec<(String, Vec<[String; 3]>)>,
        name: String,
    );
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct AssembledInterfaceSolution {
    pub connectors: Vec<Quantity>,
}

/* -------------------------------------------------------------------------- */

#[derive(Debug, PartialEq, Clone, Default)]
pub struct AssembledModelSolution {
    pub name: String,
    pub variables: Vec<Quantity>,
    pub interfaces: Vec<AssembledInterfaceSolution>,
    pub submodels: Vec<AssembledModelSolution>,
}

pub trait ModelSolution {
    fn disassemble(assembled: AssembledModelSolution) -> Self;
    fn table(&self) -> Table;
    fn collect_sections(&self, into: &mut Vec<(String, Vec<[String; 3]>)>);
}

/* -------------------------------------------------------------------------- */

pub struct SolvedSystem {
    models: Vec<AssembledModelSolution>,
}

impl SolvedSystem {
    pub fn get<'s, B: ModelBuilder>(
        &self,
        builder: B,
    ) -> <B::Builds as Model>::Solution {
        let mut current = &self.models;
        let path = builder.path();
        let mut iter = path.0.iter();

        for component in iter.by_ref().take(path.0.len() - 1) {
            current = &current[*component].submodels;
        }

        let sol = &current[*iter.next().unwrap()];
        <B::Builds as Model>::Solution::disassemble(sol.clone())
    }
}

/* -------------------------------------------------------------------------- */

impl System {
    pub fn solve(&self, solver: impl Solver) -> SolvedSystem {
        let mut knowns = AHashMap::new();
        let mut guesses = AHashMap::new();

        for (id, assoc) in self.var_association.borrow().iter() {
            let var = self.model(&id.path).variables[id.idx];

            match assoc {
                Associated::Guess(quantity) => {
                    guesses.insert(var, quantity.value().clone());
                }
                Associated::Binding(expr) => {
                    if let Some(c) =
                        try { expr.as_single()?.as_leaf()?.as_constant()? }
                    {
                        knowns.insert(var, c.quantity().value().clone());
                    } else if let Some(qty) =
                        try { expr.as_single()?.as_leaf()?.as_quantity()? }
                    {
                        knowns.insert(var, qty.value().clone());
                    }
                }
            }
        }

        for (id, assoc) in self.conn_association.borrow().iter() {
            let conn = self.model(&id.interface.path).interfaces
                [id.interface.idx]
                .connectors[id.idx];

            match assoc {
                Associated::Guess(quantity) => {
                    guesses.insert(conn.variable, quantity.value().clone());
                }
                Associated::Binding(expr) => {
                    if let Some(c) =
                        try { expr.as_single()?.as_leaf()?.as_constant()? }
                    {
                        knowns.insert(
                            conn.variable,
                            c.quantity().value().clone(),
                        );
                    } else if let Some(qty) =
                        try { expr.as_single()?.as_leaf()?.as_quantity()? }
                    {
                        knowns.insert(conn.variable, qty.value().clone());
                    }
                }
            }
        }

        let models = self.models.clone().into_inner();
        let compiled = self.clone().compile();

        for mut block in compiled.blocks.into_iter().rev() {
            let bindings = knowns
                .iter()
                .map(|(var, val)| (var.0, val.clone().into()))
                .collect();

            block.retain_mut(|resid| {
                if let Some(subbed) = resid.substituted(&bindings) {
                    *resid = subbed;
                }

                resid.simplified() != Expr::from(0.0)
            });

            if block.is_empty() {
                continue;
            }

            if block.iter().map(|resid| resid.symbols()).flatten().count() == 0
            {
                // Found a block that has equations but relates no symbols.
                // This indicates a soundness hole in the solver, as residuals equal to 0 are removed before checking.
                // If there is some residual function left which is not zero but also takes no arguments, its unsolvable
                // Unsolvability is checked by bipartite matching so this is a soundness issue
                panic!("This should never happen")
            }

            let solution = solver.solve(block, &guesses).unwrap();
            knowns.extend(solution);

            // propagate solutions over equality connectors
            for (a, adj) in &*self.adjacency.borrow() {
                for (i, a_conn) in self.model(&a.path).interfaces[a.idx]
                    .connectors
                    .iter()
                    .enumerate()
                {
                    if a_conn.condition != Condition::Equal {
                        continue;
                    }

                    let Some(known) = knowns.get(&a_conn.variable).cloned()
                    else {
                        continue;
                    };

                    for b in adj {
                        let b_conn = &self.model(&b.path).interfaces[b.idx]
                            .connectors[i];
                        knowns.insert(b_conn.variable, known.clone());
                    }
                }
            }
        }

        let mut solved = SolvedSystem { models: Vec::new() };

        for model in models {
            solved.models.push(model.solution(&knowns));
        }

        solved
    }
}
