use core::panic;
use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell},
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
    expr::{
        self, Binding, Expr,
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
pub mod solve;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Interface {
    fn new(name: &str) -> Self;
    fn assemble(self) -> AssembledInterface;
}

pub trait Solution {
    fn disassemble(assembled: AssembledSolution) -> Self;
}

pub trait Model: Relations + Clone {
    type Solution: Solution;
    type Builder<'s>;

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

pub trait InterfaceArrayExt<I: Interface> {
    fn connect(self, other: &InterfaceBuilder<I>);
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
pub struct AssembledSolution {
    pub values: Vec<Value>,
    pub subsolutions: Vec<AssembledSolution>,
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
pub struct ModelPath(Vec<usize>);

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct System {
    models: RefCell<Vec<AssembledModel>>,
    connections: RefCell<HashMap<InterfaceId, HashSet<InterfaceId>>>,
    bindings: RefCell<HashMap<VariableId, Expr>>,
}

// pub struct AssembledSystem {
//     knowns: HashMap<Variable, Expr>,
//     equations: Vec<Equation>,
// }

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CompiledSystem {
    blocks: Vec<Vec<Equation>>,
}

pub struct SolvedSystem {
    solutions: Vec<AssembledSolution>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Variable(Symbol);

#[derive(Clone)]
pub struct VariableBuilder<'s> {
    id: VariableId,
    system: &'s System,
}

#[derive(Clone)]
pub struct InterfaceBuilder<'s, I: Interface> {
    id: InterfaceId,
    system: &'s System,
    phantom: PhantomData<I>,
}

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    Equal,
    Conserved,
    // Transported,
}

/* ---------------------------------- IMPLS --------------------------------- */

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

impl<'s> VariableBuilder<'s> {
    pub fn variable(&self) -> Variable {
        self.system.assembled(&self.id.path).variables[self.id.idx]
    }

    pub fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        let system = self.system;
        let var = self.variable();
        assert_eq!(
            expr.unit().expect("Tried to bind expr with invalid dimension"),
            var.symbol().unit(),
            "Tried to bind an expression with different units to a variable"
        );
        system.bindings.borrow_mut().insert(self.id.clone(), expr);
    }
}

impl<'s, I: Interface> InterfaceBuilder<'s, I> {
    pub fn connect(&self, other: &InterfaceBuilder<I>) {
        let system = self.system;

        system
            .connections
            .borrow_mut()
            .entry(self.id.clone())
            .or_default()
            .insert(other.id.clone());

        system
            .connections
            .borrow_mut()
            .entry(other.id.clone())
            .or_default()
            .insert(self.id.clone());
    }
}

impl<'s, I: Interface, const N: usize> InterfaceArrayExt<I>
    for [&InterfaceBuilder<'s, I>; N]
{
    fn connect(self, other: &InterfaceBuilder<I>) {
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

    pub fn add<'s, M: Model>(&'s self, model: M) -> M::Builder<'s>
    where
        Self: 's, {
        let assembled = model.assemble();
        let assembled_path = {
            let mut models = self.models.borrow_mut();
            models.push(assembled);
            ModelPath(vec![models.len() - 1])
        };

        M::builder(assembled_path, &*self)
    }

    pub fn assembled<'s>(
        &'s self,
        path: &ModelPath,
    ) -> std::cell::Ref<'s, AssembledModel> {
        let mut current = self.models.borrow();
        let mut iter = path.0.iter();

        for component in iter.by_ref().take(path.0.len() - 1) {
            current = Ref::map(current, |c| &c[*component].submodels);
        }

        Ref::map(current, |c| &c[*iter.next().unwrap()])
    }

    pub fn compile(self) -> CompiledSystem {
        let mut equations = Vec::new();

        let connections = self.connections.borrow();
        let bindings = self.bindings.borrow();

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
                &self.assembled(&first_id.path).interfaces[first_id.idx];

            for (i, first_conn) in first_interface.connectors.iter().enumerate()
            {
                match first_conn.condition {
                    Condition::Equal => {
                        for other_id in &component[1..] {
                            let other_interface = &self
                                .assembled(&other_id.path)
                                .interfaces[other_id.idx];
                            let other_conn = &other_interface.connectors[i];
                            equations.push(relation! { first_conn.variable = other_conn.variable });
                        }
                    }
                    Condition::Conserved => {
                        let terms = component
                            .iter()
                            .map(|id| {
                                let interface = &self
                                    .assembled(&id.path)
                                    .interfaces[id.idx];
                                Expr::from(interface.connectors[i].variable)
                            })
                            .collect_vec();

                        equations.push(relation!(Variadic::Add(terms) = 0));
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

        for eq in &mut equations {
            let mut ctx = SimplifyContext::new();
            *eq = relation! {
                eq.lhs().simplify(&mut ctx) = eq.rhs().simplify(&mut ctx)
            }
        }

        let bindings = bindings
            .iter()
            .map(|(var_id, expr)| {
                Binding::new(
                    self.assembled(&var_id.path).variables[var_id.idx].symbol(),
                    expr.simplify(&mut SimplifyContext::new()),
                )
            })
            .collect_vec();

        let eqs = equations.iter().filter_map(|eq| {
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

        CompiledSystem { blocks: dependency_graph.sccs() }
    }
}

impl<'s> VariableBuilder<'s> {
    pub fn new(system: &'s System, id: VariableId) -> Self {
        Self { id, system }
    }
}

impl<'s, I: Interface> InterfaceBuilder<'s, I> {
    pub fn new(system: &'s System, id: InterfaceId) -> Self {
        Self { id, system, phantom: PhantomData }
    }
}

/* -------------------------------------------------------------------------- */

#[cfg(test)]
mod test {
    use engi_macros::{Interface, Model, relations};

    use crate as engi;
    use crate::{
        expr::ops::{exp, real},
        model::{
            Condition, Connector, InterfaceArrayExt, Model, Relations, System,
            Variable,
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

        let compiled = system.compile();
        panic!("{:#?}", compiled)
        // println!("{}", solution.get(bjt))
    }
}
