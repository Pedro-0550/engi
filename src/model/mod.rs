use core::panic;
use std::{
    any::{Any, TypeId},
    cell::{Cell, OnceCell, Ref, RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use derive_more::From;
use engi_macros::{relation, relations};
use itertools::{Either, Itertools};
use num::complex::Complex;

use crate::{
    self as engi,
    core::{
        graph::{BipartiteGraph, DirectedGraph},
        value::Value,
    },
    expr::{self, Expr, Node},
    model::{
        eq::{Constraint, Equation},
        solve::Solver,
    },
    simplify::{Simplify, SimplifyContext},
    symbol::{self, Symbol},
    symbols,
    units::{Dimensioned, Quantity, Unit, si::Hz},
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;
pub mod solve;

/* --------------------------------- TRAITS --------------------------------- */

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
}

pub trait InterfaceSolution {
    fn disassemble(assembled: AssembledInterfaceSolution) -> Self;
}

pub trait ModelBuilder {}

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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AssembledModel {
    pub name: String,
    pub equations: Vec<Equation>,
    pub constraints: Vec<Constraint>,
    pub variables: Vec<Variable>,
    pub interfaces: Vec<AssembledInterface>,
    pub submodels: Vec<AssembledModel>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AssembledModelSolution {
    pub variables: Vec<Quantity>,
    pub interfaces: Vec<AssembledInterfaceSolution>,
    pub submodels: Vec<AssembledModelSolution>,
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct System {
    models: RefCell<Vec<AssembledModel>>,
    adjacency: RefCell<HashMap<InterfaceId, HashSet<InterfaceId>>>,
    conn_association: RefCell<HashMap<ConnectorId, Associated>>,
    var_association: RefCell<HashMap<VariableId, Associated>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CompiledSystem {
    blocks: Vec<Vec<Expr>>,
}

pub struct SolvedSystem {
    solutions: Vec<AssembledModelSolution>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Variable(Symbol);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VariableBuilder<'s> {
    id: VariableId,
    system: &'s System,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConnectorBuilder<'s> {
    id: ConnectorId,
    system: &'s System,
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
        let None = self
            .system
            .conn_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Binding(expr.into()))
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
            .insert(self.id.clone(), Associated::Guess(qty.into()))
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
        let None = self
            .system
            .var_association
            .borrow_mut()
            .insert(self.id.clone(), Associated::Binding(expr.into()))
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
            .insert(self.id.clone(), Associated::Guess(qty.into()))
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

    pub fn model(&self, path: &ModelPath) -> Ref<AssembledModel> {
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
        let var_assoc = self.var_association.borrow();
        let conn_assoc = self.conn_association.borrow();

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
                            equations.push(relation! { first_conn.variable = other_conn.variable });
                        }
                    }
                    Condition::Conserved => {
                        let terms = component
                            .iter()
                            .map(|id| {
                                let interface =
                                    &self.model(&id.path).interfaces[id.idx];
                                Expr::from(interface.connectors[i].variable)
                            })
                            .collect();

                        equations.push(relation!(Node::Add(terms) = 0));
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

        let bindings = var_assoc
            .iter()
            .filter_map(|(var_id, assoc)| match assoc {
                Associated::Binding(expr) => Some((
                    self.model(&var_id.path).variables[var_id.idx].symbol(),
                    expr.simplify(),
                )),
                _ => None,
            })
            .chain(conn_assoc.iter().filter_map(|(conn_id, assoc)| {
                match assoc {
                    Associated::Binding(expr) => Some((
                        self.model(&conn_id.interface.path).interfaces
                            [conn_id.interface.idx]
                            .connectors[conn_id.idx]
                            .variable()
                            .symbol(),
                        expr.simplify(),
                    )),
                    _ => None,
                }
            }))
            .collect();

        let residuals = equations.iter().filter_map(|eq| {
            let resid = eq.residual();

            loop {
                let step = resid.substitute(&bindings);
                if step == resid {
                    break;
                }
                resid = step
            }

            let resid = resid.simplify();

            if resid == 0 { None } else { Some(resid) }
        });

        let mut incidence = BipartiteGraph::new();

        for eq in residuals {
            incidence.add_left(eq.clone());

            for symb in eq.symbols() {
                incidence.add_right(Variable::new(symb));
                incidence.add_edge(eq.clone(), Variable::new(symb));
            }
        }

        let matching = incidence.maximum_matching();

        if matching.size() != incidence.left_count() {
            panic!(
                "Not every equation can be assigned a variable: {} equations, {} matched
                Unmatched equations: {:#?}",
                incidence.left_count(),
                matching.size(),
                matching.unmatched_left(incidence.left_nodes()).map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
            );
        }

        if matching.size() != incidence.right_count() {
            panic!(
                "Not every variable can be assigned an equation: {} variables, {} matched
                Unmatched variables: {:#?}",
                incidence.right_count(),
                matching.size(),
                matching.unmatched_right(incidence.right_nodes()).map(|x| x.symbol().to_string()).collect::<Vec<_>>().join(", ")
            );
        }

        let mut dependency_graph = DirectedGraph::new();

        for (eq, var) in matching.edges() {
            dependency_graph.add_node(eq.clone());

            for neighbor in incidence.right_neighbors(var).unwrap() {
                if neighbor == eq {
                    continue;
                }
                dependency_graph.add_edge(eq.clone(), neighbor.clone());
            }
        }

        CompiledSystem { blocks: dependency_graph.sccs() }
    }

    pub fn solve(self, solver: impl Solver) -> AssembledModelSolution {
        // let mut knowns = self
        //     .var_bindings
        //     .borrow()
        //     .iter()
        //     .filter_map(|(from, to)| {
        //         let var = self.model(&from.path).variables[from.idx];

        //         to.node()
        //             .as_quantity()
        //             .and_then(|qty| Some((var, qty.value().clone())))
        //             .or(to.node().as_constant().and_then(|c| {
        //                 Some((var, c.quantity().value().clone()))
        //             }))
        //     })
        //     .collect_vec();

        // let mut knowns = var_assoc
        //     .iter()
        //     .filter_map(|(var_id, assoc)| match assoc {
        //         Associated::Binding(expr) => Some(Binding::new(
        //             self.model(&var_id.path).variables[var_id.idx].symbol(),
        //             expr.simplify(&mut SimplifyContext::new()),
        //         )),
        //         _ => None,
        //     })
        //     .chain(conn_assoc.iter().filter_map(|(conn_id, assoc)| {
        //         match assoc {
        //             Associated::Binding(expr) => Some(Binding::new(
        //                 self.model(&conn_id.interface.path).interfaces
        //                     [conn_id.interface.idx]
        //                     .connectors[conn_id.idx]
        //                     .variable()
        //                     .symbol(),
        //                 expr.simplify(&mut SimplifyContext::new()),
        //             )),
        //             _ => None,
        //         }
        //     }))
        //     .collect_vec();

        // let guesses = self
        //     .var_guesses
        //     .borrow()
        //     .iter()
        //     .map(|(var, qty)| {
        //         let var = self.model(&var.path).variables[var.idx];
        //         (var, qty.value().clone())
        //     })
        //     .collect();

        let mut knowns = HashMap::new();
        let mut guesses = HashMap::new();

        for (id, assoc) in self.var_association.borrow().iter() {
            let var = self.model(&id.path).variables[id.idx];

            match assoc {
                Associated::Guess(quantity) => {
                    guesses.insert(var, quantity.value().clone());
                }
                Associated::Binding(expr) => {
                    if let Some(c) = expr.node().as_constant() {
                        knowns.insert(var, c.quantity().value().clone());
                    } else if let Some(qty) = expr.node().as_quantity() {
                        knowns.insert(var, qty.value().clone());
                    }
                }
                _ => (),
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
                    if let Some(c) = expr.node().as_constant() {
                        knowns.insert(
                            conn.variable,
                            c.quantity().value().clone(),
                        );
                    } else if let Some(qty) = expr.node().as_quantity() {
                        knowns.insert(conn.variable, qty.value().clone());
                    }
                }
                _ => (),
            }
        }

        let compiled = self.compile();

        for block in compiled.blocks.iter().rev() {
            let bindings = knowns
                .iter()
                .map(|(var, val)| (var.0, val.clone().into()))
                .collect();

            let block = block
                .into_iter()
                .map(|resid| resid.substitute(&bindings))
                .collect_vec();

            println!(
                "solving [{}]",
                block.iter().map(|x| x.to_string()).join(",\n   ")
            );

            let solution = solver.solve(block, &guesses).unwrap();
            knowns.extend(solution);
        }

        for (Variable(sym), val) in knowns {
            println!("{sym} -> {:.3e}", val.as_scalar().unwrap());
        }
        todo!()
    }
}

/* -------------------------------------------------------------------------- */

#[cfg(test)]
mod test {
    use engi_macros::{Interface, Model, relations};

    use crate as engi;
    use crate::{
        expr::{exp, real},
        model::{
            Condition, Connector, InterfaceArrayExt, InterfaceBuilder, Model,
            Relations, System, Variable,
            eq::{Constraint, Equation},
            solve::NloptSolver,
        },
        symbol::constants::{kB, q},
        units::si::*,
    };

    #[derive(Interface, Clone)]
    pub struct ElectricalPin {
        #[connect(cond = Condition::Conserved, unit = A, desc = "Pin current")]
        i: Connector,

        #[connect(cond = Condition::Equal, unit = V, desc = "Pin voltage")]
        v: Connector,
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct ElectricalPort {
        #[interface]
        p: ElectricalPin,

        #[interface]
        n: ElectricalPin,

        #[var(unit = V, desc = "P-N potential")]
        v: Variable,

        #[var(unit = A, desc = "Port current")]
        i: Variable,
    }

    impl Relations for ElectricalPort {
        fn equations(&self) -> Vec<Equation> {
            let ElectricalPort { p, n, v, i } = self;

            relations! {
                p.i + n.i = 0;
                i = p.i;
                v = p.v - n.v;
            }
        }
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Interface, Clone)]
    pub struct ThermalPort {
        #[connect(cond = Condition::Equal, unit = W, desc = "Transferred power")]
        p: Connector,
        #[connect(cond = Condition::Equal, unit = K, desc = "Transferred temperature")]
        t: Connector,
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct JunctionThermal {
        #[var(unit = K, desc = "Ambient temperature")]
        t_a: Variable,

        #[var(unit = K, desc = "Case temperature")]
        t_c: Variable,

        #[var(unit = K / W, desc = "Junction-case thermal resistance")]
        rθ_jc: Variable,

        #[var(unit = K / W, desc = "Case-ambient thermal resistance")]
        rθ_ca: Variable,

        #[interface]
        pub port: ThermalPort,
    }

    impl Relations for JunctionThermal {
        fn equations(&self) -> Vec<Equation> {
            let JunctionThermal { t_a, t_c, rθ_jc, rθ_ca, port } = self;

            relations! [
                port.t - t_c = rθ_jc * port.p;
                t_c - t_a = rθ_ca * port.p;
            ]
        }
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct StaticBjt {
        #[var(unit = A, desc = "Reverse saturation current")]
        i_s: Variable,

        /* -------------------------------------------------------------------------- */
        #[var(unit = V, desc = "Base-emitter voltage")]
        v_be: Variable,
        #[var(unit = V, desc = "Base-collector voltage")]
        v_bc: Variable,
        #[var(unit = V, desc = "Collector-emitter voltage")]
        v_ce: Variable,
        #[var(unit = V, desc = "Thermal voltage")]
        v_t: Variable,

        /* -------------------------------------------------------------------------- */
        #[var(desc = "Forward current gain")]
        β_f: Variable,
        #[var(desc = "Reverse current gain")]
        β_r: Variable,

        /* -------------------------------------------------------------------------- */
        #[interface]
        pub b: ElectricalPin,

        #[interface]
        pub c: ElectricalPin,

        #[interface]
        pub e: ElectricalPin,

        #[interface]
        pub thermal: ThermalPort,
    }

    impl Relations for StaticBjt {
        fn equations(&self) -> Vec<Equation> {
            let StaticBjt {
                i_s,
                v_be,
                v_bc,
                v_ce,
                v_t,
                β_f,
                β_r,
                b,
                c,
                e,
                thermal,
            } = self;

            relations! {
                v_t = kB * thermal.t / q;
                v_be = b.v - e.v;
                v_bc = b.v - c.v;
                v_ce = c.v - e.v;

                thermal.p = v_be * b.i + v_ce * c.i;

                c.i = i_s * (exp(v_be / v_t) - exp(v_bc / v_t) - (exp(v_bc / v_t) - 1) / β_r);
                b.i = i_s * ((exp(v_be / v_t) - 1) / β_f + (exp(v_bc / v_t) - 1) / β_r);
                e.i = b.i + c.i;
            }
        }
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct Impedance {
        #[var(unit = Ω, desc = "Complex impedance")]
        z: Variable,
        #[model]
        port: ElectricalPort,
        // #[interface]
        // thermal: ThermalPort,
    }

    impl Relations for Impedance {
        fn equations(&self) -> Vec<Equation> {
            let Impedance { z, port } = self;
            relations! {
                port.v = port.i * z;
                // thermal.p = real(port.v * port.i)
            }
        }

        fn constraints(&self) -> Vec<Constraint> {
            let Impedance { z, .. } = self;
            relations! {
                real(z) > 0;
            }
        }
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct Ground {
        #[interface]
        pin: ElectricalPin,
    }

    impl Relations for Ground {
        fn equations(&self) -> Vec<Equation> {
            let Ground { pin } = self;
            relations! {
                pin.v = 0;
            }
        }
    }

    /* -------------------------------------------------------------------------- */
    #[derive(Model, Clone)]
    pub struct IdealSupply {
        #[var(unit = V, desc = "Supply output voltage")]
        v: Variable,
        #[model]
        out: ElectricalPort,
    }

    impl Relations for IdealSupply {
        fn equations(&self) -> Vec<Equation> {
            let IdealSupply { out, v } = self;
            relations! {
                out.v = v;
            }
        }
    }

    /* -------------------------------------------------------------------------- */
    #[test]
    fn modeling() {
        let system = System::new();

        let v_b = system.add(IdealSupply::new("v_b"));
        let v_c = system.add(IdealSupply::new("v_c"));
        let q1 = system.add(StaticBjt::new("q1"));
        let r_c = system.add(Impedance::new("r_c"));
        let r_b = system.add(Impedance::new("r_b"));
        let gnd = system.add(Ground::new("gnd"));
        let q1_thermal = system.add(JunctionThermal::new("q1_thermal"));

        r_c.port.p.connect(&v_c.out.p);
        r_c.port.n.connect(&q1.c);
        r_c.port.i.guess(6e-3 * A);
        q1.c.i.guess(6e-3 * A);

        r_b.port.p.connect(&v_b.out.p);
        r_b.port.n.connect(&q1.b);
        r_b.z.guess(600e3 * Ω);

        r_b.port.i.guess(60e-6 * A);
        q1.b.i.guess(60e-6 * A);

        [&v_c.out.n, &v_b.out.n, &q1.e].connect(&gnd.pin);

        q1.thermal.connect(&q1_thermal.port);

        v_b.v.bind(5 * V);
        v_c.v.bind(12 * V);

        r_c.z.bind(10e3 * Ω);
        q1.v_ce.bind(v_c.v / 2);
        q1.v_be.guess(0.4 * V);
        q1.v_t.guess(25e-3 * V);
        q1.β_f.bind(100);
        q1.β_r.bind(10);
        q1.i_s.bind(100e-9 * A);

        // bjt_thermal.t_a.bind(25.0 * DEG_C);
        q1_thermal.rθ_jc.bind(10 * K / W);
        q1_thermal.rθ_ca.bind(20 * K / W);
        q1_thermal.t_a.bind(300 * K);
        q1_thermal.t_c.guess(350 * K);
        q1.thermal.t.guess(360 * K);

        let solution =
            system.solve(NloptSolver::new(nlopt::Algorithm::TNewtonPrecond));
        // panic!("{:#?}", compiled)
        // println!("{}", solution.get(bjt))
    }
}
