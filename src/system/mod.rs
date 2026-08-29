use core::panic;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
};

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
        eq::{Constraint, Equation},
        var::{Variable, VariableBuilder},
    },
};

/* --------------------------------- MODULES -------------------------------- */

pub mod eq;
pub mod var;

/* --------------------------------- TRAITS --------------------------------- */

// pub trait Systems {}

// macro_rules! impl_systems {
//     ($($T:ident),*) => {
//         impl<$($T: System),*> Systems for ($($T,)*) {}
//     };
// }

// all_tuples!(impl_systems, 0, 24, T);

// impl<S: System> Systems for S {}

pub trait Interface {
    fn variables(&self) -> Vec<InterfaceVariable>;
}

pub trait Model: Constraints + Equations {
    type FieldAccess;
    type Solution;

    fn interfaces(&self) -> Vec<Box<dyn Interface>>;
    fn variables(&self) -> Vec<Variable>;
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

pub struct InterfaceVariable {
    condition: Condition,
}

pub struct InterfaceId(usize);
pub struct ModelId(usize);

pub struct ModelHandle<M: Model> {
    fields: M::FieldAccess,
    id: ModelId,
}

pub struct InterfaceHandle<I: Interface> {
    id: InterfaceId,
    _phantom: PhantomData<I>,
}

pub struct Connection {
    a: InterfaceId,
    b: InterfaceId,
}

pub struct System {
    models: HashMap<ModelId, Box<dyn ErasedModel>>,
    connections: Vec<Connection>,
}

// pub trait Hierarchy {
//     type Parent: System;
//     type Root: System = <Self::Parent as System>::Parent;
// }

// pub trait System: Equations + Variables + Constraints + Hierarchy
// where
//     Self: 'static + Sized, {
//     type Solution;
//     type Root: System = <<Self as Hierarchy>::Parent as System>::Parent;

//     fn id() -> SystemId {
//         SystemId(TypeId::of::<Self>())
//     }

//     fn collect_variables(&self, into: &mut Vec<Variable>);
//     fn collect_equations(&self, into: &mut Vec<Equation>);
//     fn collect_constraints(&self, into: &mut Vec<Constraint>);

//     fn structure(
//         &self,
//         deps: <Self as Equations>::Dependencies,
//     ) -> SystemStructure {
//         // Equation <-> Variable
//         let mut incidence = BipartiteGraph::new();

//         let equations = self.equations(deps);
//         let variables = self.variables();

//         for _ in &variables {
//             incidence.add_right();
//         }

//         for eq in &equations {
//             let eq_node = incidence.add_left();

//             for symbol in eq.symbols() {
//                 let var_node = RightNode::new(
//                     variables
//                         .iter()
//                         .position(|x| x.symbol() == symbol)
//                         .unwrap(),
//                 );

//                 incidence.add_edge(eq_node, var_node);
//             }
//         }

//         SystemStructure { incidence, equations, variables }
//     }
// }

// pub trait ErasedSystem: Any {
//     fn id(&self) -> SystemId;

//     fn collect_variables(&self, into: &mut Vec<Variable>);
//     fn collect_equations(&self, into: &mut Vec<Equation>);
//     fn collect_constraints(&self, into: &mut Vec<Constraint>);

//     fn structure(&self) -> SystemStructure;

//     fn as_any(&self) -> &dyn Any;
//     fn as_any_mut(&mut self) -> &mut dyn Any;
//     fn into_any(self: Box<Self>) -> Box<dyn Any>;
// }

// /* --------------------------------- STRUCTS -------------------------------- */
// pub struct SystemContext {}

// pub struct SystemStructure {
//     incidence: BipartiteGraph,
//     equations: Vec<Equation>,
//     variables: Vec<Variable>,
// }

// #[derive(Clone, Copy, PartialEq, Hash)]
// pub struct SystemId(TypeId);

// pub enum Solvability {
//     Symbolic,
//     Numeric,
// }

// pub struct Solution {
//     solved: HashMap<Variable, Scalar>,
//     observed: HashMap<Variable, Scalar>,
//     residuals: HashMap<Equation, f64>,
// }

pub enum Condition {
    Equal,
    Conserved,
    Transported,
}

/* ---------------------------------- IMPLS --------------------------------- */

// TODO filter by known when relev ant

// impl SystemStructure {
//     fn simplify(&mut self) {
//         let matching = self.incidence.maximum_matching();
//         if !matching.is_perfect() {
//             panic!()
//         }

//         let mut dependency_graph = Vec::new();

//         for (eq_node, var_node) in matching.edges() {
//             let eq = &self.equations[eq_node.index()];
//             let var = &self.variables[var_node.index()];
//             for sym in eq.symbols() {
//                 if sym == var.symbol() {
//                     continue;
//                 }

//                 let sym_idx = self
//                     .variables
//                     .iter()
//                     .position(|x| x.symbol() == sym)
//                     .unwrap();

//                 dependency_graph.push((sym_idx, var_node.index()));
//             }
//         }

//         panic!("{:#?}", dependency_graph);
//     }
// }

/* -------------------------------------------------------------------------- */

mod model_based_large_signal_bjt {
    use crate::system::var::Variable;

    #[derive(Interface)]
    pub struct Pin {
        #[connect(Condition::Conserved)]
        #[var(unit = A, desc = "Pin current")]
        i: Variable,

        #[connect(Condition::Equal)]
        #[var(unit = V, desc = "Pin voltage")]
        v: Variable,
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct Port {
        p: Pin,
        n: Pin,

        #[var(unit = V, desc = "P-N potential")]
        v: Variable,

        #[var(unit = V, desc = "Port current")]
        i: Variable,
    }

    impl Equations for Port {
        fn equations(&self) -> Vec<Equation> {
            let port_fields!() = self;
            equations![p.i = n.i, i = p.i, v = p.v - n.v]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Interface)]
    pub struct ThermalPort {
        #[connect(Condition::Equal)]
        #[var(unit = W, desc = "Transferred power")]
        p: Variable,
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct SemiconductorThermal {
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

        pub port: ThermalPort,
    }

    impl Equations for SemiconductorThermal {
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
        pub base: Pin,
        pub collector: Pin,
        pub emitter: Pin,
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
        port: Port,
        thermal: ThermalPort,
    }

    impl Equations for Impedance {
        fn equations(&self) -> Vec<Equation> {
            let impedance_fields!() = self;
            equations![port.v = port.i * z, thermal.p = real(port.v * port.i)]
        }
    }

    /* -------------------------------------------------------------------------- */

    #[derive(Model)]
    pub struct Ground {
        pin: Pin,
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
        out: Port,
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

        let thermal = system.add(StaticThermal::default(), "q1_thermal");

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

        let solution = system.solve();
    }
}
