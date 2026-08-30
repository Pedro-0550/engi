use core::panic;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
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
    system::{
        eq::{Constraint, Equation},
        var::Variable,
    },
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Interface {
    fn connectors(&self) -> Vec<Connector>;
}

pub trait ModelBuilder {
    fn new(id: ModelId) -> Self;
}

pub trait Model: Constraints + Equations {
    type Builder: ModelBuilder;
    type Solution;

    fn interfaces(&self) -> Vec<Box<dyn Interface>>;
    fn variables(&self) -> Vec<Variable>;
    fn submodels(&self) -> Vec<Box<dyn ErasedModel>>;
    fn builder(&self, system: &System) -> Self::Builder;
    fn erased(self) -> Box<dyn ErasedModel> {
        todo!()
    }
}

pub trait ErasedModel: Any {
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

/* --------------------------------- STRUCTS -------------------------------- */

pub struct Connector {
    condition: Condition,
    unit: Unit,
    desc: String,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct InterfaceId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ModelId(usize);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct VariableId(usize);

struct Connection {
    a: InterfaceId,
    b: InterfaceId,
}

struct Binding {
    var: Variable,
    val: Value,
}

struct SystemInner {
    models: Vec<Box<dyn ErasedModel>>,
    interfaces: Vec<Box<dyn Interface>>,
    variables: Vec<Variable>,
    connections: Vec<Connection>,
    bindings: HashMap<VariableId, Value>,
}

type SystemInnerRef = Rc<RefCell<SystemInner>>;

pub struct System(SystemInnerRef);

pub struct Variable {
    symbol: Symbol,
}

pub struct VariableBuilder<'m, M: Model> {
    id: VariableId,
    model: &'m M::Builder,
}

pub struct InterfaceBuilder<'m, M: Model> {
    id: InterfaceId,
    model: &'m M::Builder,
}

/* ---------------------------------- ENUMS --------------------------------- */

pub enum Condition {
    Equal,
    Conserved,
    Transported,
}

#[derive(From)]
pub enum Value {
    Set(),
    Matrix(),
    Scalar(Scalar),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl<M: Model> Constraints for M {
    default fn constraints(&self) -> Vec<Constraint> {
        Vec::new()
    }
}

impl System {
    const NEXT_MODEL_ID: AtomicUsize = AtomicUsize::new(1);

    fn add<M: Model>(&mut self, model: M) -> &mut M::Builder {
        let id = ModelId(Self::NEXT_MODEL_ID.fetch_add(1, Ordering::SeqCst));
        let builder = model.builder(&*self);

        self.models.borrow_mut().insert(id, model.erased());

        &mut builder
    }
}

/* -------------------------------------------------------------------------- */

mod model_based_large_signal_bjt {
    use engi_macros::equations;

    use crate as engi;
    use crate::{
        dimension::{other::t, si::*},
        system::{
            Connector, Constraints, Equations,
            eq::{Constraint, Equation},
            var::Variable,
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
            let electrical_port_fields!() = self;
            equations![p.i = n.i, i = p.i, v = p.v - n.v]
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

        #[model]
        pub port: ThermalPort,
    }

    impl Equations for SemiThermal {
        fn equations(&self) -> Vec<Equation> {
            let static_thermal_fields!() = self;
            equations![t_j - t_c = rθ_jc * p, t_c - t_a = rθ_ca * p]
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
        pub base: ElectricalPin,

        #[interface]
        pub collector: ElectricalPin,

        #[interface]
        pub emitter: ElectricalPin,

        #[interface]
        pub thermal: ThermalPort,
    }

    impl Equations for StaticBjt {
        fn equations(&self) -> Vec<Equation> {
            let static_bjt_fields!() = self;

            equations![
                v_t = kB * T / q,
                v_be = base.v - emitter.v,
                v_bc = base.v - collector.v,
                v_ce = collector.v - emitter.v,
                collector.i = i_s
                    * (exp(v_be / v_t)
                        - exp(v_bc / v_t)
                        - (exp(v_bc / v_t) - 1) / β_r),
                base.i = i_s
                    * ((exp(v_be / v_t) - 1) / β_f
                        + (exp(v_bc / v_t) - 1) / β_r),
                emitter.i = i_s
                    * (exp(v_be / v_t) - exp(v_bc / v_t)
                        + (exp(v_be / v_t) - 1) / β_f),
                p_d = v_be * i_b + v_ce * i_c
            ]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct Impedance {
        #[var(unit = Ω, desc = "Complex impedance")]
        z: Variable,
        #[interface]
        port: ElectricalPort,
        #[interface]
        thermal: ThermalPort,
    }

    impl Equations for Impedance {
        fn equations(&self) -> Vec<Equation> {
            let impedance_fields!() = self;
            equations![port.v = port.i * z, thermal.p = re(port.v * port.i)]
        }
    }

    impl Constraints for Impedance {
        fn constraints(&self) -> Vec<Constraint> {
            equations![re(self.z) > 0]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct Ground {
        pin: ElectricalPin,
    }

    impl Equations for Ground {
        fn equations(&self) -> Vec<Equation> {
            let ground_fields!() = self;
            equations![gnd.v = 0]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct IdealSupply {
        out: ElectricalPort,
        #[var(unit = V, desc = "Supply output voltage")]
        v: Variable,
    }

    impl Equations for IdealSupply {
        fn equations(&self) -> Vec<Equation> {
            let ideal_supply_fields!() = self;
            equations![port.v = v]
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
        let thermal = system.add(SemiThermal::default(), "q1_thermal");

        bjt.thermal.connect(thermal.port);

        r_c.port.p.connect(v_c.port.p);
        r_b.port.n.connect(bjt.collector);

        r_b.port.p.connect(v_b.port.p);
        r_b.port.n.connect(bjt.base);

        [v_c.port.n, v_b.port.n, bjt.emitter].connect(gnd.pin);

        v_b.v.bind(2 * V);
        v_c.v.bind(12 * V);

        r_c.z.bind(1e3 * Ω);
        bjt.v_ce.bind(6 * V);

        thermal.t_amb.bind((25 + 273.15) * K);
        thermal.rθ_jc.bind(10 * K / W);
        thermal.rθ_ca.bind(20 * K / W);

        let solution = system.solve();
        println!("{}", solution.get(bjt))
    }
}
