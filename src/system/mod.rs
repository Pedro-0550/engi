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
use itertools::Either;
use num::complex::Complex;
use variadics_please::all_tuples;

use crate::{
    core::{
        graph::{BipartiteGraph, RightNode},
        scalar::{I, Scalar},
    },
    dimension::{Quantity, Unit, si::Hz},
    expr::{Expr, ops::sin},
    symbol::Symbol,
    symbols,
    system::eq::{Constraint, Equation},
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Interface {
    fn connectors(&self) -> Vec<Connector>;
}

pub trait ModelBuilder {}

pub trait Model: Constraints + Equations {
    type Builder: ModelBuilder;
    type Solution;

    fn register(self, system: System) -> Self::Builder;
    fn erased(self) -> Box<dyn ErasedModel>
    where
        Self: Sized, {
        todo!()
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

#[derive(Debug, Clone, Copy)]
pub struct Connector {
    condition: Condition,
    variable: Variable,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct InterfaceId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ModelId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct VariableId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct Connection {
    a: InterfaceId,
    b: InterfaceId,
}

struct Binding {
    var: Variable,
    val: Value,
}

#[derive(Default)]
struct SystemInner {
    models: Vec<Box<dyn ErasedModel>>,
    interfaces: Vec<Box<dyn Interface>>,
    variables: Vec<Variable>,
    connections: HashSet<Connection>,
    bindings: HashMap<VariableId, Value>,
}

#[derive(Clone)]
pub struct System(Rc<RefCell<SystemInner>>);

#[derive(Debug, Clone, Copy, Hash)]
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

#[derive(Debug, Clone, Copy)]
pub enum Condition {
    Equal,
    Conserved,
    Transported,
}

#[derive(From, Clone, Copy)]
pub enum Value {
    Set(),
    Matrix(),
    Scalar(Scalar),
    // TODO: temp
    Temp(Quantity),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Connector {
    pub fn variable(&self) -> Variable {
        self.variable
    }

    pub fn condition(&self) -> Condition {
        self.condition
    }
}

impl Connection {
    fn new(a: InterfaceId, b: InterfaceId) -> Self {
        Self { a, b }
    }
}

impl VariableBuilder {
    fn bind(&self, val: impl Into<Value>) {
        let val = val.into();
        self.system.0.borrow_mut().bindings.insert(self.id, val);
    }
}

impl InterfaceBuilder {
    fn connect(&self, other: &InterfaceBuilder) {
        let mut inner = self.system.0.borrow_mut();
        let connection = Connection::new(self.id, other.id);

        if inner.connections.contains(&Connection::new(other.id, self.id))
            || inner.connections.contains(&connection)
        {
            return;
        }

        inner.connections.insert(connection);
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

    pub fn add<M: Model>(&self, model: M, name: &str) -> M::Builder {
        model.register(self.clone())
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

    fn add_model(&mut self, model: impl Model) -> ModelId {
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
    use engi_macros::{Model, relations};

    use crate as engi;
    use crate::{
        dimension::si::*,
        expr::ops::exp,
        system::{
            Connector, Constraints, Equations, InterfaceArrayExt, System,
            Variable,
            eq::{Constraint, Equation},
        },
    };

    #[derive(Interface)]
    pub struct ElectricalPin {
        #[connect(cond = Condition::Conserved, unit = A, desc = "Pin current")]
        i: Connector,

        #[connect(cond = Condition::Conserved, unit = V, desc = "Pin voltage")]
        v: Connector,
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
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
                p.i = n.i;
                i = p.i;
                v = p.v - n.v;
            }
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Interface)]
    pub struct ThermalPort {
        #[connect(cond = Condition::Equal, unit = W, desc = "Transferred power")]
        p: Connector,
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct SemiThermal {
        #[var(unit = K, desc = "Ambient temperature")]
        t_a: Variable,

        #[var(unit = K, desc = "Junction temperature")]
        t_j: Variable,

        #[var(unit = K, desc = "Case temperature")]
        t_c: Variable,

        #[var(unit = K / W, desc = "Junction-case thermal resistance")]
        rθ_jc: Variable,

        #[var(unit = K / W, desc = "Case-ambient thermal resistance")]
        rθ_ca: Variable,

        #[interface]
        pub port: ThermalPort,
    }

    impl Equations for SemiThermal {
        fn equations(&self) -> Vec<Equation> {
            let SemiThermal { t_a, t_j, t_c, rθ_jc, rθ_ca, port } = self;

            relations! [
                t_j - t_c = rθ_jc * port.p;
                t_c - t_a = rθ_ca * port.p;
            ]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
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
        #[var(unit = W, desc = "Power dissipation")]
        p_d: Variable,

        /* -------------------------------------------------------------------------- */
        #[interface]
        pub b: ElectricalPin,

        #[interface]
        pub c: ElectricalPin,

        #[interface]
        pub e: ElectricalPin,

        #[model]
        pub thermal: SemiThermal,
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
                p_d,
                b,
                c,
                e,
                thermal,
            } = self;

            relations! {
                v_t = kB * thermal.t_j / q;
                v_be = b.v - e.v;
                v_bc = b.v - c.v;
                v_ce = c.v - e.v;

                p_d = v_be * b.i + v_ce * c.i;

                c.i = i_s * (exp(v_be / v_t) - exp(v_bc / v_t) - (exp(v_bc / v_t) - 1) / β_r);
                b.i = i_s * ((exp(v_be / v_t) - 1) / β_f + (exp(v_bc / v_t) - 1) / β_r);
                e.i = b.i + c.i;
            }
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct Impedance {
        #[var(unit = Ω, desc = "Complex impedance")]
        z: Variable,
        #[model]
        port: ElectricalPort,
        #[interface]
        thermal: ThermalPort,
    }

    impl Equations for Impedance {
        fn equations(&self) -> Vec<Equation> {
            let Impedance { z, port, thermal } = self;
            relations! {
                port.v = port.i * z;
                thermal.p = re(port.v * port.i)
            }
        }
    }

    impl Constraints for Impedance {
        fn constraints(&self) -> Vec<Constraint> {
            let Impedance { z, .. } = self;
            relations! {
                re(z) > 0;
            }
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
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

    #[derive(Model)]
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

    fn main() {
        let system = System::new();

        let v_b = system.add(IdealSupply::default(), "v_b");
        let v_c = system.add(IdealSupply::default(), "v_c");
        let bjt = system.add(StaticBjt::default(), "q1");
        let r_c = system.add(Impedance::default(), "r_c");
        let r_b = system.add(Impedance::default(), "r_b");
        let gnd = system.add(Ground::default(), "gnd");

        r_c.port.p.connect(&v_c.out.p);
        r_b.port.n.connect(&bjt.c);

        r_b.port.p.connect(&v_b.out.p);
        r_b.port.n.connect(&bjt.b);

        [&v_c.out.n, &v_b.out.n, &bjt.e].connect(&gnd.pin);

        v_b.v.bind(2 * V);
        v_c.v.bind(12 * V);

        r_c.z.bind(1e3 * Ω);
        bjt.v_ce.bind(6 * V);

        bjt.thermal.t_a.bind((25.0 + 273.15) * K);
        bjt.thermal.rθ_jc.bind(10 * K / W);
        bjt.thermal.rθ_ca.bind(20 * K / W);

        let solution = system.solve();
        println!("{}", solution.get(bjt))
    }
}
