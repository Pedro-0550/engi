use core::panic;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::{HashMap, HashSet},
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
    expr::{
        self, Expr,
        ops::{Variadic, sin},
    },
    model::eq::{Constraint, Equation},
    simplify::{Simplify, SimplifyContext},
    symbol::{self, Symbol},
    symbols,
    units::{Dimensioned, Quantity, Unit, si::Hz},
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Interface {
    fn connectors(&self) -> Vec<Connector>;
    fn new(name: &str) -> Self;

    fn erased(self) -> Box<dyn ErasedInterface>
    where
        Self: Sized + Any, {
        Box::new(self)
    }
}

pub trait ErasedInterface: Any {
    fn connectors(&self) -> Vec<Connector>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub trait ModelBuilder {}

pub trait Model: Constraints + Equations + Clone {
    type Builder: ModelBuilder;
    type Solution;

    fn register(self, system: System) -> Self::Builder;
    fn new(name: &str) -> Self;

    fn erased(self) -> Box<dyn ErasedModel>
    where
        Self: Sized + Any, {
        Box::new(self)
    }
}

pub trait ErasedModel: Any + Constraints + Equations {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub trait Constraints {
    fn constraints(&self) -> Vec<Constraint>;
}

pub trait Equations {
    fn equations(&self) -> Vec<Equation>;
}

pub trait InterfaceArrayExt {
    fn connect(self, other: &InterfaceBuilder);
}

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Connector {
    variable: Variable,
    condition: Condition,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct InterfaceId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ModelId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct VariableId(usize);

#[derive(Default)]
struct SystemInner {
    models: Vec<Box<dyn ErasedModel>>,
    interfaces: Vec<Box<dyn ErasedInterface>>,
    variables: Vec<Variable>,
    connections: HashMap<InterfaceId, HashSet<InterfaceId>>,
    bindings: HashMap<VariableId, Expr>,
}

#[derive(Clone)]
pub struct System(Rc<RefCell<SystemInner>>);

pub struct AssembledSystem {
    knowns: HashMap<Variable, Expr>,
    equations: Vec<Equation>,
}

pub struct AnalyzedSystem {
    blocks: Vec<Vec<Equation>>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Variable {
    symbol: Symbol,
}

#[derive(Clone)]
pub struct VariableBuilder {
    id: VariableId,
    system: System,
}

#[derive(Clone)]
pub struct InterfaceBuilder {
    id: InterfaceId,
    system: System,
}

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    Equal,
    Conserved,
    // Transported,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl<M> ErasedModel for M
where
    M: Model + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl<I> ErasedInterface for I
where
    I: Interface + 'static,
{
    fn connectors(&self) -> Vec<Connector> {
        self.connectors()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
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

// impl Connection {
//     fn new(a: InterfaceId, b: InterfaceId) -> Self {
//         Self { a, b }
//     }

//     fn transpose(&self) -> Self {
//         Self { a: self.b, b: self.a }
//     }
// }

impl VariableBuilder {
    pub fn variable(&self) -> Variable {
        self.system.0.borrow().variables[self.id.0]
    }

    pub fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        let mut inner = self.system.0.borrow_mut();
        assert_eq!(
            expr.unit().expect("Tried to bind expr with invalid dimension"),
            inner.variables[self.id.0].symbol().unit(),
            "Tried to bind an expression with different units to a variable"
        );
        inner.bindings.insert(self.id, expr);
    }
}

impl InterfaceBuilder {
    pub fn connect(&self, other: &InterfaceBuilder) {
        let mut inner = self.system.0.borrow_mut();

        inner
            .connections
            .entry(self.id)
            .or_insert_with(HashSet::new)
            .insert(other.id);

        inner
            .connections
            .entry(other.id)
            .or_insert_with(HashSet::new)
            .insert(self.id);
    }
}

impl<const N: usize> InterfaceArrayExt for [&InterfaceBuilder; N] {
    fn connect(self, other: &InterfaceBuilder) {
        for interface in self {
            interface.connect(other);
        }
    }
}

impl<M: Model> Constraints for M {
    default fn constraints(&self) -> Vec<Constraint> {
        Vec::new()
    }
}

impl Variable {
    pub fn new(symbol: Symbol) -> Self {
        Self { symbol }
    }

    pub fn symbol(&self) -> Symbol {
        self.symbol
    }
}

impl System {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(SystemInner::default())))
    }

    pub fn add<M: Model>(&self, model: M) -> M::Builder {
        model.register(self.clone())
    }

    pub fn assemble(self) -> AssembledSystem {
        let inner = self.0.take();
        let mut equations = Vec::new();

        /* -------------------------------------------------------------------------- */

        for model in &inner.models {
            equations.extend(model.equations());
        }

        /* -------------------------------------------------------------------------- */

        let mut visited = HashSet::new();

        fn ordered_pair(
            a: InterfaceId,
            b: InterfaceId,
        ) -> (InterfaceId, InterfaceId) {
            if a.0 < b.0 { (a, b) } else { (b, a) }
        }

        for start_id in 0..inner.interfaces.len() {
            let start = InterfaceId(start_id);

            if !visited.insert(start) {
                continue;
            }

            let mut stack = vec![start];
            let mut component = Vec::new();

            while let Some(id) = stack.pop() {
                component.push(id);

                if let Some(adjacent) = inner.connections.get(&id) {
                    for &next in adjacent {
                        if visited.insert(next) {
                            stack.push(next);
                        }
                    }
                }
            }

            /* -------------------------------------------------------------------------- */

            let mut explored_edges = HashSet::new();

            for &a_id in &component {
                let a = &inner.interfaces[a_id.0];

                let Some(adjacent) = inner.connections.get(&a_id) else {
                    continue;
                };

                for &b_id in adjacent {
                    if !explored_edges.insert(ordered_pair(a_id, b_id)) {
                        continue;
                    }

                    let b = &inner.interfaces[b_id.0];

                    for (a_conn, b_conn) in
                        a.connectors().iter().zip(b.connectors())
                    {
                        if a_conn.condition() == Condition::Equal {
                            equations.push(relation! {
                                a_conn.variable() = b_conn.variable()
                            });
                        }
                    }
                }
            }

            /* -------------------------------------------------------------------------- */

            let mut conserved_terms: HashMap<usize, Vec<Expr>> = HashMap::new();

            for &id in &component {
                let interface = &inner.interfaces[id.0];

                for (i, connector) in interface.connectors().iter().enumerate()
                {
                    if connector.condition() == Condition::Conserved {
                        conserved_terms
                            .entry(i)
                            .or_default()
                            .push(connector.variable().into());
                    }
                }
            }

            for terms in conserved_terms.into_values() {
                if terms.len() > 1 {
                    equations.push(relation! {
                        Variadic::Add(terms) = 0.0
                    });
                }
            }
        }

        for eq in &mut equations {
            let mut ctx = SimplifyContext::new();
            *eq = relation! {
                eq.lhs().simplify(&mut ctx) = eq.rhs().simplify(&mut ctx)
            }
        }

        let knowns: HashMap<Variable, Expr> = inner
            .bindings
            .iter()
            .map(|(var_id, expr)| {
                (
                    inner.variables[var_id.0],
                    expr.simplify(&mut SimplifyContext::new()),
                )
            })
            .collect();

        AssembledSystem { knowns, equations }
    }
}

impl AssembledSystem {
    pub fn analyze(self) -> AnalyzedSystem {
        let bindings = &self
            .knowns
            .iter()
            .map(|(var, val)| expr::Binding::new(var.symbol(), val.into()))
            .collect_vec();

        let eqs = self.equations.iter().filter_map(|eq| {
            let mut lhs = eq.lhs().clone();

            loop {
                let step = lhs.substitute(&bindings);
                if step == lhs {
                    break;
                }
                lhs = step
            }

            let mut rhs = eq.rhs().clone();

            loop {
                let step = rhs.substitute(&bindings);
                if step == rhs {
                    break;
                }
                rhs = step
            }

            if lhs == rhs { None } else { Some(Equation::new(lhs, rhs)) }
        });

        let mut incidence = BipartiteGraph::new();

        for eq in eqs {
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

        panic!("{:#?}", dependency_graph.sccs());

        AnalyzedSystem { blocks: dependency_graph.sccs() }
    }
}

impl SystemInner {
    fn add_variable(&mut self, var: Variable) -> VariableId {
        let id = VariableId(self.variables.len());
        self.variables.push(var);
        id
    }

    fn add_interface(
        &mut self,
        interface: impl Interface + 'static,
    ) -> InterfaceId {
        let id = InterfaceId(self.interfaces.len());
        self.interfaces.push(Box::new(interface));
        id
    }

    fn add_model(&mut self, model: impl Model + 'static) -> ModelId {
        let id = ModelId(self.models.len());
        self.models.push(model.erased());
        id
    }
}

impl VariableBuilder {
    pub fn new(system: System, id: VariableId) -> Self {
        Self { id, system }
    }
}

impl InterfaceBuilder {
    pub fn new(system: System, id: InterfaceId) -> Self {
        Self { id, system }
    }
}

/* -------------------------------------------------------------------------- */

mod model_based_large_signal_bjt {
    use engi_macros::{Interface, Model, relations};

    use crate as engi;
    use crate::{
        expr::ops::{exp, real},
        model::{
            Condition, Connector, Constraints, Equations, InterfaceArrayExt,
            Model, System, Variable,
            eq::{Constraint, Equation},
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

        #[var(unit = V, desc = "Port current")]
        i: Variable,
    }

    impl Equations for ElectricalPort {
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

    impl Equations for JunctionThermal {
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

    impl Equations for StaticBjt {
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

    impl Equations for Impedance {
        fn equations(&self) -> Vec<Equation> {
            let Impedance { z, port } = self;
            relations! {
                port.v = port.i * z;
                // thermal.p = real(port.v * port.i)
            }
        }
    }

    impl Constraints for Impedance {
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

    impl Equations for Ground {
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

    impl Equations for IdealSupply {
        fn equations(&self) -> Vec<Equation> {
            let IdealSupply { out, v } = self;
            relations! {
                out.v = v;
            }
        }
    }

    /* -------------------------------------------------------------------------- */

    #[test]
    fn main() {
        let system = System::new();

        let v_b = system.add(IdealSupply::new("v_b"));
        let v_c = system.add(IdealSupply::new("v_c"));
        let bjt = system.add(StaticBjt::new("q1"));
        let r_c = system.add(Impedance::new("r_c"));
        let r_b = system.add(Impedance::new("r_b"));
        let gnd = system.add(Ground::new("gnd"));
        let bjt_thermal = system.add(JunctionThermal::new("q1_thermal"));

        r_c.port.p.connect(&v_c.out.p);
        r_c.port.n.connect(&bjt.c);

        r_b.port.p.connect(&v_b.out.p);
        r_b.port.n.connect(&bjt.b);

        [&v_c.out.n, &v_b.out.n, &bjt.e].connect(&gnd.pin);

        bjt.thermal.connect(&bjt_thermal.port);

        v_b.v.bind(2 * V);
        v_c.v.bind(12 * V);

        r_c.z.bind(1e3 * Ω);
        bjt.v_ce.bind(v_c.v / 2);
        bjt.β_f.bind(100);
        bjt.β_r.bind(10);
        bjt.i_s.bind(10e-9 * A);

        // bjt_thermal.t_a.bind(25.0 * DEG_C);
        bjt_thermal.rθ_jc.bind(10 * K / W);
        bjt_thermal.rθ_ca.bind(20 * K / W);
        bjt_thermal.t_a.bind(300 * K);

        let solution = system.assemble().analyze();
        panic!()
        // println!("{}", solution.get(bjt))
    }
}
