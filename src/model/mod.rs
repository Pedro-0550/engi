use core::panic;
use std::{
    cell::{Ref, RefCell},
    collections::HashSet,
    fmt::Debug,
    hash::Hash,
    mem,
};

use ahash::{AHashMap, AHashSet};
use engi_macros::relation;
use itertools::Itertools;
use petgraph::{
    algo::{maximum_matching, tarjan_scc},
    graphmap::{DiGraphMap, UnGraphMap},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tabled::Table;

use crate::{
    self as engi,
    core::{indexed::Indexed, value::Value},
    expr::Expr,
    id,
    model::eq::{Constraint, Equation},
    symbol::{Symbol, constants::q},
    unit::Quantity,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;
pub mod lm;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Solver {
    type Error: Debug;

    fn solve(
        &self,
        residuals: Vec<Expr>,
        guesses: &AHashMap<Variable, Value>,
    ) -> Result<Vec<(Variable, Value)>, Self::Error>;
}

pub trait InterfaceBuilder {
    fn id(&self) -> &InterfaceId;
    fn system(&self) -> &System;

    fn connect(&self, other: &Self) {
        // A -> B
        self.system()
            .adjacency
            .borrow_mut()
            .entry(self.id().clone())
            .or_default()
            .insert(other.id().clone());
        // B -> A
        self.system()
            .adjacency
            .borrow_mut()
            .entry(other.id().clone())
            .or_default()
            .insert(self.id().clone());
        // if you think about it, in computers every graph is directed
    }
}

pub trait Interface {
    type Solution: InterfaceSolution;
    type Builder<'s>: InterfaceBuilder;

    fn new(name: &str) -> Self;
    fn builder<'s>(id: InterfaceId, system: &'s System) -> Self::Builder<'s>;

    fn assemble(self) -> AssembledInterface;
}

pub trait ModelSolution {
    fn disassemble(assembled: AssembledModelSolution) -> Self;
    fn table(&self) -> Table;
    fn collect_sections(&self, into: &mut Vec<(String, Vec<[String; 3]>)>);
}

pub trait InterfaceSolution {
    fn disassemble(assembled: AssembledInterfaceSolution) -> Self;
    fn collect_sections(
        &self,
        into: &mut Vec<(String, Vec<[String; 3]>)>,
        name: String,
    );
}

pub trait ModelBuilder {
    type Builds: Model;
    fn path(&self) -> &ModelPath;
}

pub trait Model: Relations + Clone {
    type Solution: ModelSolution;
    type Builder<'s>: ModelBuilder;

    fn new(name: &str) -> Self;
    fn builder<'s>(path: ModelPath, system: &'s System) -> Self::Builder<'s>;

    fn assemble(self) -> AssembledModel;
}

pub trait Relations {
    fn constraints(&self) -> Vec<Constraint> {
        vec![]
    }
    fn equations(&self) -> Vec<Equation> {
        vec![]
    }
}

pub trait InterfaceArrayExt<I: InterfaceBuilder> {
    fn connect(self, other: &I);
}

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Debug, Clone)]
pub struct AssembledModel {
    pub name: String,
    pub equations: Vec<Equation>,
    pub constraints: Vec<Constraint>,
    pub variables: Vec<Variable>,
    pub interfaces: Vec<AssembledInterface>,
    pub submodels: Vec<AssembledModel>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct AssembledModelSolution {
    pub name: String,
    pub variables: Vec<Quantity>,
    pub interfaces: Vec<AssembledInterfaceSolution>,
    pub submodels: Vec<AssembledModelSolution>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct AssembledInterfaceSolution {
    pub connectors: Vec<Quantity>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AssembledInterface {
    pub connectors: Vec<Connector>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Connector {
    variable: Variable,
    condition: Condition,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct VariableId {
    path: ModelPath,
    idx: usize,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct InterfaceId {
    path: ModelPath,
    idx: usize,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct ConnectorId {
    interface: InterfaceId,
    idx: usize,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct ModelPath(Vec<usize>);

#[derive(Default, Debug, Clone)]
pub struct System {
    models: RefCell<Vec<AssembledModel>>,
    adjacency: RefCell<AHashMap<InterfaceId, AHashSet<InterfaceId>>>,
    conn_association: RefCell<AHashMap<ConnectorId, Associated>>,
    var_association: RefCell<AHashMap<VariableId, Associated>>,
}

#[derive(Debug, Clone)]
pub struct CompiledSystem {
    blocks: Vec<Vec<Expr>>,
}

pub struct SolvedSystem {
    models: Vec<AssembledModelSolution>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Variable(Symbol);

#[derive(Debug, Clone)]
pub struct VariableBuilder<'s> {
    id: VariableId,
    system: &'s System,
}

#[derive(Debug, Clone)]
pub struct ConnectorBuilder<'s> {
    id: ConnectorId,
    system: &'s System,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Associated {
    Guess(Quantity),
    Binding(Expr),
}

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    Equal,
    Conserved,
    // Transported,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Default for Associated {
    fn default() -> Self {
        Self::Guess(Quantity::default())
    }
}

impl VariableId {
    pub fn new(path: ModelPath, idx: usize) -> Self {
        Self { path, idx }
    }
}

impl InterfaceId {
    pub fn new(path: ModelPath, idx: usize) -> Self {
        Self { path, idx }
    }
}

impl ConnectorId {
    pub fn new(interface: InterfaceId, idx: usize) -> Self {
        Self { interface, idx }
    }
}

impl<M: Model> Relations for M {
    default fn constraints(&self) -> Vec<Constraint> {
        vec![]
    }

    default fn equations(&self) -> Vec<Equation> {
        vec![]
    }
}

impl Connector {
    pub fn new(variable: Variable, condition: Condition) -> Self {
        Self { variable, condition }
    }

    pub fn variable(&self) -> Variable {
        self.variable
    }

    pub fn condition(&self) -> Condition {
        self.condition
    }
}

impl<'s> ConnectorBuilder<'s> {
    pub fn new(id: ConnectorId, system: &'s System) -> Self {
        Self { id, system }
    }

    pub fn connector(&self) -> Connector {
        self.system.model(&self.id.interface.path).interfaces
            [self.id.interface.idx]
            .connectors[self.id.idx]
    }

    pub fn bind(&self, expr: impl Into<Expr>) {
        let mut expr = expr.into();

        if let Some(qty) = try { expr.as_single()?.as_leaf()?.as_quantity()? } {
            expr = qty.clone().normalize().into()
        }

        let None = self
            .system
            .conn_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Binding(expr))
        else {
            panic!(
                "Connector already has associated guess or binding, cannot add a bind"
            )
        };
    }

    pub fn guess(&self, qty: impl Into<Quantity>) {
        let None = self
            .system
            .conn_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Guess(qty.into().normalize()))
        else {
            panic!(
                "Connector already has associated guess or binding, cannot add a guess"
            )
        };
    }
}

impl<'s> VariableBuilder<'s> {
    pub fn new(id: VariableId, system: &'s System) -> Self {
        Self { id, system }
    }

    pub fn variable(&self) -> Variable {
        self.system.model(&self.id.path).variables[self.id.idx]
    }

    pub fn bind(&self, expr: impl Into<Expr>) {
        let mut expr = expr.into();

        if let Some(qty) = try { expr.as_single()?.as_leaf()?.as_quantity()? } {
            expr = qty.clone().normalize().into()
        }

        let None = self
            .system
            .var_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Binding(expr))
        else {
            panic!(
                "Variable already has associated guess or binding, cannot add a bind"
            )
        };
    }

    pub fn guess(&self, qty: impl Into<Quantity>) {
        let None = self
            .system
            .var_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Guess(qty.into().normalize()))
        else {
            panic!(
                "Variable already has associated guess or binding, cannot add a guess"
            )
        };
    }
}

impl<I: InterfaceBuilder, const N: usize> InterfaceArrayExt<I> for [&I; N] {
    fn connect(self, other: &I) {
        for interface in self {
            interface.connect(other);
        }
    }
}

impl Variable {
    pub fn new(symbol: Symbol) -> Self {
        Self(symbol)
    }

    pub fn symbol(&self) -> Symbol {
        self.0
    }
}

impl ModelPath {
    fn with(&self, components: &[usize]) -> Self {
        let mut buf = Vec::new();
        buf.extend(&self.0);
        buf.extend(components);
        ModelPath(buf)
    }
}

impl AssembledInterface {
    fn solution(
        self,
        knowns: &AHashMap<Variable, Value>,
    ) -> AssembledInterfaceSolution {
        let mut sol = AssembledInterfaceSolution::default();

        for conn in self.connectors {
            sol.connectors
                .push(knowns[&conn.variable].clone() * conn.variable.0.unit());
        }

        sol
    }
}

impl AssembledModel {
    fn solution(
        self,
        knowns: &AHashMap<Variable, Value>,
    ) -> AssembledModelSolution {
        let mut sol = AssembledModelSolution::default();

        for var in self.variables {
            sol.variables.push(knowns[&var].clone() * var.0.unit());
        }

        for interface in self.interfaces {
            sol.interfaces.push(interface.solution(knowns));
        }

        for submodel in self.submodels {
            sol.submodels.push(submodel.solution(knowns));
        }
        sol.name = self.name;

        sol
    }
}

impl SolvedSystem {
    fn get<'s, B: ModelBuilder>(
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

impl System {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<'s, M: Model>(&'s self, model: M) -> M::Builder<'s> {
        let assembled = model.assemble();
        let mut models = self.models.borrow_mut();
        let assembled_path = ModelPath(vec![models.len()]);
        models.push(assembled);

        M::builder(assembled_path, self)
    }

    pub fn model<'s>(&'s self, path: &ModelPath) -> Ref<'s, AssembledModel> {
        let mut current = self.models.borrow();
        let mut iter = path.0.iter();

        for component in iter.by_ref().take(path.0.len() - 1) {
            current = Ref::map(current, |c| &c[*component].submodels);
        }

        Ref::map(current, |c| &c[*iter.next().unwrap()])
    }

    pub fn compile(self) -> CompiledSystem {
        let mut equations = Vec::new();

        let connections = self.adjacency.borrow();

        /* -------------------------------------------------------------------------- */

        let mut visited = HashSet::new();

        for start_id in connections.keys() {
            if !visited.insert(start_id.clone()) {
                continue;
            }

            let mut component = vec![start_id.clone()];
            let mut stack = vec![start_id.clone()];

            while let Some(curr) = stack.pop() {
                if let Some(adjacent) = connections.get(&curr) {
                    for neighbor in adjacent {
                        if visited.insert(neighbor.clone()) {
                            component.push(neighbor.clone());
                            stack.push(neighbor.clone());
                        }
                    }
                }
            }

            let first_id = &component[0];
            let first_interface =
                &self.model(&first_id.path).interfaces[first_id.idx];

            for (i, first_conn) in first_interface.connectors.iter().enumerate()
            {
                match first_conn.condition {
                    Condition::Equal => {
                        for other_id in &component[1..] {
                            let other_interface = &self
                                .model(&other_id.path)
                                .interfaces[other_id.idx];
                            let other_conn = &other_interface.connectors[i];

                            // Associations are propagated on equality
                            let first_conn_id = ConnectorId {
                                interface: first_id.clone(),
                                idx: i,
                            };
                            let other_conn_id = ConnectorId {
                                interface: other_id.clone(),
                                idx: i,
                            };

                            let mut assoc = self.conn_association.borrow_mut();
                            let first_assoc = assoc.get(&first_conn_id);
                            let other_assoc = assoc.get(&other_conn_id);

                            match (first_assoc, other_assoc) {
                                (None, Some(other)) => {
                                    let other = other.clone();
                                    assoc.insert(first_conn_id, other);
                                }
                                (Some(first), None) => {
                                    let first = first.clone();
                                    assoc.insert(other_conn_id, first);
                                }
                                (Some(first), Some(other)) => {
                                    assert_eq!(*first, *other, "
                                        Associations on connectors with equality constraints propagate,
                                        and as such a chain of equality-connected cannot have different associations");
                                }
                                _ => (),
                            };

                            equations.push(relation! { first_conn.variable = other_conn.variable });
                        }
                    }
                    Condition::Conserved => {
                        let mut terms = component.iter().map(|id| {
                            let interface =
                                &self.model(&id.path).interfaces[id.idx];
                            interface.connectors[i].variable
                        });

                        let mut term_sum = Expr::new();
                        let init = term_sum.push(terms.next().unwrap().into());
                        let root_term = terms.fold(init, |a, v| {
                            let b = term_sum.push(v.into());
                            term_sum.edit().add(a, b)
                        });
                        term_sum.set_root(root_term);

                        equations.push(relation!(term_sum = 0));
                    }
                }
            }
        }

        fn collect_equations(
            model: &mut AssembledModel,
            equations: &mut Vec<Equation>,
        ) {
            equations.append(&mut model.equations);

            for submodel in &mut model.submodels {
                collect_equations(submodel, equations);
            }
        }

        for model in &mut *self.models.borrow_mut() {
            collect_equations(model, &mut equations);
        }

        let var_assoc = self.var_association.borrow().clone();
        let conn_assoc = self.conn_association.borrow().clone();

        let bindings = var_assoc
            .into_iter()
            .filter_map(|(var_id, assoc)| match assoc {
                Associated::Binding(expr) => Some((
                    self.model(&var_id.path).variables[var_id.idx].symbol(),
                    expr.simplified(),
                )),
                _ => None,
            })
            .chain(conn_assoc.into_iter().filter_map(|(conn_id, assoc)| {
                match assoc {
                    Associated::Binding(expr) => Some((
                        self.model(&conn_id.interface.path).interfaces
                            [conn_id.interface.idx]
                            .connectors[conn_id.idx]
                            .variable()
                            .symbol(),
                        expr.simplified(),
                    )),
                    _ => None,
                }
            }))
            .collect();

        id!(ResidualId);

        #[derive(Copy, Hash, Eq, PartialEq, Clone, PartialOrd, Ord)]
        enum Node {
            Residual(ResidualId),
            Variable(Symbol),
        }

        let mut residuals: Indexed<Vec<_>, ResidualId> = equations
            .par_iter()
            .filter_map(|eq| {
                let mut resid = eq.residual();

                loop {
                    let Some(step) = resid.substituted(&bindings) else {
                        break;
                    };
                    resid = step;
                }

                resid = resid.simplified();

                if try { resid.as_single()?.as_leaf()?.as_quantity()? }
                    .is_some_and(|x| *x.value() == 0)
                {
                    None
                } else {
                    Some(resid)
                }
            })
            .collect::<Vec<_>>()
            .into();

        let mut incidence = UnGraphMap::<Node, ()>::new();

        for (id, resid) in residuals.iter() {
            let r = incidence.add_node(Node::Residual(id));

            for symb in resid.symbols() {
                let v = incidence.add_node(Node::Variable(symb));
                incidence.add_edge(r, v, ());
            }
        }

        let matching = maximum_matching(&incidence);

        // if matching. {
        //     panic!(
        //         "Not every equation could be assigned to a variable: {} equations, {} matched
        //         Unmatched equations: {:#?}
        //         This indicates the system is overdetermined.
        //         You need to unbind a variable so the system has a new degree of freedom.",
        //         incidence.left_count(),
        //         matching.size(),
        //         matching.unmatched_left(incidence.left_nodes()).map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        //     );
        // }

        // if matching.size() != incidence.right_count() {
        //     panic!(
        //         "Not every variable could be assigned to an equation: {} variables, {} matched
        //         Unmatched variables: {:#?}
        //         This indicates the system is underdetermined.
        //         You should verify if you entered all the equations, or bind a constant value to another variable.",
        //         incidence.right_count(),
        //         matching.size(),
        //         matching.unmatched_right(incidence.right_nodes()).map(|x| x.symbol().to_string()).collect::<Vec<_>>().join(", ")
        //     );
        // }

        if !matching.is_perfect() {
            panic!()
        }

        let mut dependency_graph = DiGraphMap::<ResidualId, ()>::new();

        for (a, b) in matching.edges() {
            let (sym, resid) = match (a, b) {
                (Node::Residual(r), Node::Variable(s)) => (s, r),
                (Node::Variable(s), Node::Residual(r)) => (s, r),
                _ => unreachable!(),
            };

            dependency_graph.add_node(resid);

            for neighbor in incidence.neighbors(Node::Variable(sym)) {
                let Node::Residual(neighbor) = neighbor else {
                    unreachable!();
                };

                if neighbor == resid {
                    continue;
                }
                dependency_graph.add_edge(resid, neighbor, ());
            }
        }

        CompiledSystem {
            blocks: tarjan_scc(&dependency_graph)
                .iter()
                .map(|block| {
                    block
                        .iter()
                        .map(|id| mem::take(&mut residuals[*id]))
                        .collect_vec()
                })
                .collect_vec(),
        }
    }

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

/* -------------------------------------------------------------------------- */

#[cfg(test)]
mod test;
