#![allow(non_upper_case_globals)]

use std::f64::consts::{E, PI};

use ordered_float::OrderedFloat;

use crate::unit::{self, Dimension, Unit};

macro_rules! prefixes {
    (
        $($name:ident),*
    ) => {
        $(
            pub const ${concat(q, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(q, $name)}), base: &$name, scale: OrderedFloat(1e-30) };
            pub const ${concat(r, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(r, $name)}), base: &$name, scale: OrderedFloat(1e-27) };
            pub const ${concat(y, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(y, $name)}), base: &$name, scale: OrderedFloat(1e-24) };
            pub const ${concat(z, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(z, $name)}), base: &$name, scale: OrderedFloat(1e-21) };
            pub const ${concat(a, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(a, $name)}), base: &$name, scale: OrderedFloat(1e-18) };
            pub const ${concat(f, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(f, $name)}), base: &$name, scale: OrderedFloat(1e-15) };
            pub const ${concat(p, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(p, $name)}), base: &$name, scale: OrderedFloat(1e-12) };
            pub const ${concat(n, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(n, $name)}), base: &$name, scale: OrderedFloat(1e-9) };
            pub const ${concat(u, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(u, $name)}), base: &$name, scale: OrderedFloat(1e-6) };
            pub const ${concat(m, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(m, $name)}), base: &$name, scale: OrderedFloat(1e-3) };
            pub const ${concat(c, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(c, $name)}), base: &$name, scale: OrderedFloat(1e-2) };
            pub const ${concat(d, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(d, $name)}), base: &$name, scale: OrderedFloat(1e-1) };

            pub const ${concat(da, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(da, $name)}), base: &$name, scale: OrderedFloat(1e1) };
            pub const ${concat(h, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(h, $name)}), base: &$name, scale: OrderedFloat(1e2) };
            pub const ${concat(k, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(k, $name)}), base: &$name, scale: OrderedFloat(1e3) };
            pub const ${concat(M, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(M, $name)}), base: &$name, scale: OrderedFloat(1e6) };
            pub const ${concat(G, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(G, $name)}), base: &$name, scale: OrderedFloat(1e9) };
            pub const ${concat(T, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(T, $name)}), base: &$name, scale: OrderedFloat(1e12) };
            pub const ${concat(P, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(P, $name)}), base: &$name, scale: OrderedFloat(1e15) };
            pub const ${concat(E, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(E, $name)}), base: &$name, scale: OrderedFloat(1e18) };
            pub const ${concat(Z, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(Z, $name)}), base: &$name, scale: OrderedFloat(1e21) };
            pub const ${concat(Y, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(Y, $name)}), base: &$name, scale: OrderedFloat(1e24) };
            pub const ${concat(R, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(R, $name)}), base: &$name, scale: OrderedFloat(1e27) };
            pub const ${concat(Q, $name)}: Unit = Unit::Linear { symbol: stringify!(${concat(Q, $name)}), base: &$name, scale: OrderedFloat(1e30) };
        )*
    };
}

pub const UNITLESS: Unit = Unit::Unitless;

/* -------------------------------------------------------------------------- */

pub const s: Unit = Unit::Base { dimension: Dimension::TIME, symbol: "s" };
pub const m: Unit = Unit::Base { dimension: Dimension::LENGTH, symbol: "m" };
pub const kg: Unit = Unit::Base { dimension: Dimension::MASS, symbol: "kg" };
pub const A: Unit =
    Unit::Base { dimension: Dimension::ELECTRIC_CURRENT, symbol: "A" };
pub const K: Unit =
    Unit::Base { dimension: Dimension::TEMPERATURE, symbol: "K" };
pub const cd: Unit =
    Unit::Base { dimension: Dimension::LUMINOUS_INTENSITY, symbol: "cd" };
pub const mol: Unit =
    Unit::Base { dimension: Dimension::AMOUNT_OF_SUBSTANCE, symbol: "mol" };
pub const rad: Unit =
    Unit::Base { dimension: Dimension::DIMENSIONLESS, symbol: "rad" };
pub const sr: Unit =
    Unit::Base { dimension: Dimension::DIMENSIONLESS, symbol: "sr" };
/* -------------------------------------------------------------------------- */

pub const Hz: Unit = Unit::Derived { symbol: "Hz", base: &[(s, -1)] };
pub const N: Unit =
    Unit::Derived { symbol: "N", base: &[(kg, 1), (m, 1), (s, -2)] };
pub const Pa: Unit =
    Unit::Derived { symbol: "Pa", base: &[(kg, 1), (m, -1), (s, -2)] };
pub const J: Unit =
    Unit::Derived { symbol: "J", base: &[(kg, 1), (m, 2), (s, -2)] };
pub const W: Unit =
    Unit::Derived { symbol: "W", base: &[(kg, 1), (m, 2), (s, -3)] };
pub const C: Unit = Unit::Derived { symbol: "C", base: &[(A, 1), (s, 1)] };
pub const V: Unit =
    Unit::Derived { symbol: "V", base: &[(kg, 1), (m, 2), (s, -3), (A, -1)] };

/// Ohm
pub const Ω: Unit =
    Unit::Derived { symbol: "Ω", base: &[(kg, 1), (m, 2), (s, -3), (A, -2)] };

/// Siemens
pub const S: Unit =
    Unit::Derived { symbol: "S", base: &[(kg, -1), (m, -2), (s, 3), (A, 2)] };

/// Farad
pub const F: Unit =
    Unit::Derived { symbol: "F", base: &[(kg, -1), (m, -2), (s, 4), (A, 2)] };

/// Weber
pub const Wb: Unit =
    Unit::Derived { symbol: "Wb", base: &[(kg, 1), (m, 2), (s, -2), (A, -1)] };

/// Tesla
pub const T: Unit =
    Unit::Derived { symbol: "T", base: &[(kg, 1), (s, -2), (A, -1)] };

/// Henry
pub const H: Unit =
    Unit::Derived { symbol: "H", base: &[(kg, 1), (m, 2), (s, -2), (A, -2)] };

/* -------------------------------------------------------------------------- */

pub const min: Unit =
    Unit::Linear { symbol: "min", base: &s, scale: OrderedFloat(60.0) };
pub const hr: Unit =
    Unit::Linear { symbol: "hr", base: &s, scale: OrderedFloat(3600.0) };
pub const day: Unit =
    Unit::Linear { symbol: "d", base: &s, scale: OrderedFloat(86400.0) };
pub const inch: Unit =
    Unit::Linear { symbol: "in", base: &m, scale: OrderedFloat(0.0254) };

/// 1/1000th of an inch
pub const mil: Unit =
    Unit::Linear { symbol: "mil", base: &inch, scale: OrderedFloat(1e-3) };
pub const ft: Unit =
    Unit::Linear { symbol: "ft", base: &m, scale: OrderedFloat(0.3048) };

/// Yard
pub const yd: Unit =
    Unit::Linear { symbol: "yd", base: &m, scale: OrderedFloat(0.9144) };

/// Mile
pub const mi: Unit =
    Unit::Linear { symbol: "mi", base: &m, scale: OrderedFloat(1609.344) };

/// Nautical Mile
pub const nmi: Unit =
    Unit::Linear { symbol: "nmi", base: &m, scale: OrderedFloat(1852.0) };

/// Degree
pub const deg: Unit =
    Unit::Linear { symbol: "°", base: &rad, scale: OrderedFloat(PI / 180.0) };

/// Arc-minute
pub const arcmin: Unit =
    Unit::Linear {
        symbol: "′", base: &rad, scale: OrderedFloat(PI / 10800.0)
    };

/// Arc-second
pub const arcsec: Unit = Unit::Linear {
    symbol: "″",
    base: &rad,
    scale: OrderedFloat(PI / 648000.0),
};

/// Electron-volt
pub const eV: Unit = Unit::Linear {
    symbol: "eV",
    base: &J,
    scale: OrderedFloat(1.602176634e-19),
};

/// (small) calorie
pub const cal: Unit =
    Unit::Linear { symbol: "cal", base: &J, scale: OrderedFloat(4.184) };

/// Metric ton
pub const t: Unit =
    Unit::Linear { symbol: "t", base: &kg, scale: OrderedFloat(1000.0) };

/// Atomic mass unit
pub const u: Unit = Unit::Linear {
    symbol: "u",
    base: &kg,
    scale: OrderedFloat(1.660_539_066_60e-27),
};

/// Astronomical Unit
pub const au: Unit = Unit::Linear {
    symbol: "au",
    base: &m,
    scale: OrderedFloat(149_597_870_700.0),
};

/// Light-year
pub const ly: Unit = Unit::Linear {
    symbol: "ly",
    base: &m,
    scale: OrderedFloat(9.460_730_472_580_8e15),
};

/// Standard atmospheric pressure
pub const atm: Unit =
    Unit::Linear { symbol: "atm", base: &Pa, scale: OrderedFloat(101325.0) };

/// Bar
pub const bar: Unit =
    Unit::Linear { symbol: "bar", base: &Pa, scale: OrderedFloat(1e5) };
pub const psi: Unit = Unit::Linear {
    symbol: "psi",
    base: &Pa,
    scale: OrderedFloat(6894.757293168),
};

/// Degree Celsius
pub const oC: Unit = Unit::Affine {
    symbol: "°C",
    base: &K,
    scale: OrderedFloat(1.0),
    offset: OrderedFloat(273.15),
};

/// Degree Farenheit
pub const oF: Unit = Unit::Affine {
    symbol: "°F",
    base: &K,
    scale: OrderedFloat(5. / 9.),
    offset: OrderedFloat(459.67 * 5. / 9.),
};

/// Degree Rankine
pub const oR: Unit =
    Unit::Linear { symbol: "°Ra", base: &K, scale: OrderedFloat(5. / 9.) };

/// Bel
pub const B: Unit = Unit::Logarithmic {
    reference: &UNITLESS,
    symbol: "B",
    base: OrderedFloat(10.0),
};

/// Neper
pub const Np: Unit = Unit::Logarithmic {
    reference: &UNITLESS,
    symbol: "B",
    base: OrderedFloat(E),
};

/* -------------------------------------------------------------------------- */

prefixes!(
    s, m, A, K, mol, Hz, N, Pa, J, W, C, V, Ω, S, F, Wb, T, H, eV, rad, sr
);
