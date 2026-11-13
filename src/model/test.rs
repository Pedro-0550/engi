use engi_macros::{Interface, Model, relations};

use crate as engi;
use crate::{
    expr::{exp, real},
    model::{
        Condition, Connector, InterfaceArrayExt, InterfaceBuilder, Model,
        ModelSolution, Relations, System, Variable,
        eq::{Constraint, Equation},
        lm::LmSolver,
    },
    symbol::constants::{kB, q},
    unit::defs::*,
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
    name: String,

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
        let ElectricalPort { p, n, v, i, .. } = self;

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
    name: String,

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
        let JunctionThermal { t_a, t_c, rθ_jc, rθ_ca, port, .. } = self;

        relations! [
            port.t - t_c = rθ_jc * port.p;
            t_c - t_a = rθ_ca * port.p;
        ]
    }
}

/* -------------------------------------------------------------------------- */
#[derive(Model, Clone)]
pub struct StaticBjt {
    name: String,

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
            ..
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
    name: String,

    #[var(unit = Ω, desc = "Complex impedance")]
    z: Variable,
    #[model]
    port: ElectricalPort,
    // #[interface]
    // thermal: ThermalPort,
}

impl Relations for Impedance {
    fn equations(&self) -> Vec<Equation> {
        let Impedance { z, port, .. } = self;
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
    name: String,

    #[interface]
    pin: ElectricalPin,
}

impl Relations for Ground {
    fn equations(&self) -> Vec<Equation> {
        let Ground { pin, .. } = self;
        relations! {
            pin.v = 0;
        }
    }
}

/* -------------------------------------------------------------------------- */
#[derive(Model, Clone)]
pub struct IdealSupply {
    name: String,

    #[var(unit = V, desc = "Supply output voltage")]
    v: Variable,
    #[model]
    out: ElectricalPort,
}

impl Relations for IdealSupply {
    fn equations(&self) -> Vec<Equation> {
        let IdealSupply { out, v, .. } = self;
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
    let q1_jth = system.add(JunctionThermal::new("q1_jth"));

    r_c.port.p.connect(&v_c.out.p);
    r_c.port.n.connect(&q1.c);
    r_c.port.i.guess(6 * mA);
    q1.c.i.guess(6 * mA);

    r_b.port.p.connect(&v_b.out.p);
    r_b.port.n.connect(&q1.b);
    r_b.z.guess(500 * kΩ);

    r_b.port.i.guess(60 * µA);
    q1.b.i.guess(60 * µA);

    [&v_c.out.n, &v_b.out.n, &q1.e].connect(&gnd.pin);

    q1.thermal.connect(&q1_jth.port);

    v_b.v.bind(5 * V);
    v_c.v.bind(12 * V);

    r_c.z.bind(10 * kΩ);
    r_c.port.n.v.bind(6 * V);
    q1.v_be.guess(0.5 * V);
    q1.v_bc.guess(-5.3 * V);
    q1.v_t.guess(24 * mV);
    q1.β_f.bind(500);
    q1.β_r.bind(5);
    q1.i_s.bind(10 * fA);

    q1_jth.rθ_jc.bind(10 * K / W);
    q1_jth.rθ_ca.bind(20 * K / W);
    q1_jth.t_a.bind(21 * oC);
    q1_jth.t_c.guess(21 * oC);
    q1.thermal.t.guess(21 * oC);
    q1.thermal.p.guess(1 * mW);

    let solution = system.solve(LmSolver);
    println!(
        "{}",
        side_by_side(
            &[
                solution.get(q1).table().to_string(),
                solution.get(q1_jth).table().to_string(),
                solution.get(r_b).table().to_string(),
                solution.get(r_c).table().to_string()
            ],
            1
        )
    );
}

fn side_by_side(tables: &[String], gap: usize) -> String {
    let lines: Vec<Vec<&str>> =
        tables.iter().map(|table| table.lines().collect()).collect();

    let widths: Vec<usize> = lines
        .iter()
        .map(|table| {
            table.iter().map(|line| line.chars().count()).max().unwrap_or(0)
        })
        .collect();

    let height = lines.iter().map(Vec::len).max().unwrap_or(0);
    let separator = " ".repeat(gap);

    (0..height)
        .map(|i| {
            lines
                .iter()
                .zip(&widths)
                .map(|(table, width)| {
                    let line = table.get(i).copied().unwrap_or("");
                    format!(
                        "{line}{}",
                        " ".repeat(width - line.chars().count())
                    )
                })
                .collect::<Vec<_>>()
                .join(&separator)
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
