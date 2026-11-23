use core::panic;
use std::{
    cell::{Ref, RefCell},
    collections::HashSet,
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    mem::{self, variant_count},
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
    core::{
        indexed::{Indexed, Size},
        value::Value,
    },
    expr::Expr,
    id,
    model::{
        eq::{Constraint, Equation},
        solve::{
            AssembledInterfaceSolution, AssembledModelSolution,
            InterfaceSolution, ModelSolution,
        },
        state::{StatefulView, States},
    },
    symbol::{Symbol, constants::q},
    unit::Quantity,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod compile;
pub mod eq;
pub mod solve;
pub mod state;

mod test;

/* --------------------------------- TRAITS --------------------------------- */

pub trait InterfaceBuilder {
    type Builds: Interface;

    fn id(&self) -> &InterfaceId;
    fn system(&self) -> &System;

    fn bind(&self, template: &<Self::Builds as Interface>::Template);

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
    type Template: InterfaceTemplate;

    fn new(name: &str) -> Self;
    fn builder<'s>(id: InterfaceId, system: &'s System) -> Self::Builder<'s>;

    fn assemble(self) -> AssembledInterface;
}

pub trait InterfaceTemplate {}

pub trait InterfaceArrayExt<I: InterfaceBuilder> {
    fn connect(self, other: &I);
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AssembledInterface {
    pub connectors: Vec<Connector>,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct InterfaceId {
    path: ModelPath,
    idx: usize,
}

impl InterfaceId {
    pub fn new(path: ModelPath, idx: usize) -> Self {
        Self { path, idx }
    }
}

impl<I: InterfaceBuilder, const N: usize> InterfaceArrayExt<I> for [&I; N] {
    fn connect(self, other: &I) {
        for interface in self {
            interface.connect(other);
        }
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

#[derive(Default)]
pub struct InterfaceCursor {
    pub connector: usize,
}

/* -------------------------------------------------------------------------- */

pub trait ModelTemplate: Default + Clone {}

pub trait ModelBuilder {
    type Builds: Model;

    fn bind(&self, template: &<Self::Builds as Model>::Template);
    fn path(&self) -> &ModelPath;
}

pub trait Model {
    type Template: ModelTemplate;
    type Solution: ModelSolution;
    type Builder<'s>: ModelBuilder;

    fn new(name: &str) -> Self;
    fn builder<'s>(path: ModelPath, system: &'s System) -> Self::Builder<'s>;

    fn assemble(self) -> AssembledModel;
}

#[derive(Debug, Clone)]
pub struct AssembledModel {
    pub name: String,
    pub equations: Vec<Equation>,
    pub constraints: Vec<Constraint>,
    pub variables: Vec<Variable>,
    pub interfaces: Vec<AssembledInterface>,
    pub submodels: Vec<AssembledModel>,
}

#[derive(Default)]
pub struct ModelCursor {
    pub submodel: usize,
    pub variable: usize,
    pub interface: usize,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct ModelPath(Vec<usize>);

impl ModelPath {
    pub fn with(&self, components: &[usize]) -> Self {
        let mut buf = Vec::new();
        buf.extend(&self.0);
        buf.extend(components);
        ModelPath(buf)
    }
}

pub trait Relations: Model {
    fn constraints(&self) -> Vec<Constraint> {
        vec![]
    }
    fn equations(&self) -> Vec<Equation> {
        vec![]
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

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Connector {
    variable: Variable,
    condition: Condition,
}

#[derive(Debug, Clone)]
pub struct ConnectorBuilder<'s> {
    id: ConnectorId,
    system: &'s System,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct ConnectorId {
    interface: InterfaceId,
    idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    Equal,
    Conserved,
    // Transported,
}

impl ConnectorId {
    pub fn new(interface: InterfaceId, idx: usize) -> Self {
        Self { interface, idx }
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

/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Variable(Symbol);

#[derive(Debug, Clone)]
pub struct VariableBuilder<'s> {
    id: VariableId,
    system: &'s System,
}

#[derive(Clone, Hash, Debug, PartialEq, PartialOrd, Eq)]
pub struct VariableId {
    path: ModelPath,
    idx: usize,
}

impl VariableId {
    pub fn new(path: ModelPath, idx: usize) -> Self {
        Self { path, idx }
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

/* -------------------------------------------------------------------------- */

#[derive(Default, Debug, Clone)]
pub struct System {
    models: RefCell<Vec<AssembledModel>>,
    adjacency: RefCell<AHashMap<InterfaceId, AHashSet<InterfaceId>>>,
    conn_association: RefCell<AHashMap<ConnectorId, Associated>>,
    var_association: RefCell<AHashMap<VariableId, Associated>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Associated {
    Guess(Quantity),
    Binding(Expr),
}

impl Default for Associated {
    fn default() -> Self {
        Self::Guess(Quantity::default())
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
}
