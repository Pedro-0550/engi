#![allow(non_upper_case_globals)]

use std::f64::consts::{E, PI};

use ordered_float::OrderedFloat;

use crate::units::{self, Dimension, Node, NodeId, Unit};

macro_rules! units {
    (
        $($(#[$meta:meta])* $name:ident = $def:expr),*
    ) => {
        $(
            $(#[$meta])*
            pub const $name: Unit = NodeId(${index()});
        )*
        pub(super) const DEFAULT_UNITS: [Node; ${count($def)}] = [$(
                $def,
        )*];
    };
}

units! {
    UNITLESS = Node::Unitless,

    s = Node::Base { dimension: Dimension::TIME, symbol: "s" },
    m = Node::Base { dimension: Dimension::LENGTH, symbol: "m" },
    kg = Node::Base { dimension: Dimension::MASS, symbol: "kg" },
    A = Node::Base { dimension: Dimension::ELECTRIC_CURRENT, symbol: "A" },
    K = Node::Base { dimension: Dimension::TEMPERATURE, symbol: "K" },
    cd = Node::Base { dimension: Dimension::LUMINOUS_INTENSITY, symbol: "cd" },
    mol = Node::Base { dimension: Dimension::AMOUNT_OF_SUBSTANCE, symbol: "mol" },
    rad = Node::Base { dimension: Dimension::DIMENSIONLESS, symbol: "rad" },
    sr = Node::Base { dimension: Dimension::DIMENSIONLESS, symbol: "sr" },

    Hz = Node::Derived { symbol: "Hz", base: &[(s, -1)] },

    N = Node::Derived { symbol: "N", base: &[(kg, 1), (m, 1), (s, -2)] },
    Pa = Node::Derived { symbol: "Pa", base: &[(kg, 1), (m, -1), (s, -2)] },

    J = Node::Derived { symbol: "J", base: &[(kg, 1), (m, 2), (s, -2)] },
    W = Node::Derived { symbol: "W", base: &[(kg, 1), (m, 2), (s, -3)] },

    C = Node::Derived { symbol: "C", base: &[(A, 1), (s, 1)] },
    V = Node::Derived { symbol: "V", base: &[(kg, 1), (m, 2), (s, -3), (A, -1)] },
    /// Ohm
    Ω = Node::Derived { symbol: "Ω", base: &[(kg, 1), (m, 2), (s, -3), (A, -2)] },
    S = Node::Derived { symbol: "S", base: &[(kg, -1), (m, -2), (s, 3), (A, 2)] },

    F = Node::Derived { symbol: "F", base: &[(kg, -1), (m, -2), (s, 4), (A, 2)] },
    Wb = Node::Derived { symbol: "Wb", base: &[(kg, 1), (m, 2), (s, -2), (A, -1)] },
    T = Node::Derived { symbol: "T", base: &[(kg, 1), (s, -2), (A, -1)] },
    H = Node::Derived { symbol: "H", base: &[(kg, 1), (m, 2), (s, -2), (A, -2)] },

    /* -------------------------------------------------------------------------- */
    // NON SI

    /* Time */

    min = Node::Linear { symbol: "min", base: s, scale: OrderedFloat(60.0) },
    hr = Node::Linear { symbol: "hr", base: s, scale: OrderedFloat(3600.0) },
    day = Node::Linear { symbol: "d", base: s, scale: OrderedFloat(86400.0) },

    /* Length */

    inch = Node::Linear { symbol: "in", base: m, scale: OrderedFloat(0.0254) },
    /// 1/1000th of an inch
    mil  = Node::Linear { symbol: "mil", base: inch, scale: OrderedFloat(1e-3) },
    ft = Node::Linear { symbol: "ft", base: m, scale: OrderedFloat(0.3048) },
    /// Yard
    yd = Node::Linear { symbol: "yd", base: m, scale: OrderedFloat(0.9144) },
    /// Mile
    mi = Node::Linear { symbol: "mi", base: m, scale: OrderedFloat(1609.344) },
    /// Nautical Mile
    nmi = Node::Linear { symbol: "nmi", base: m, scale: OrderedFloat(1852.0) },

    /* Angle */

    /// Degree
    deg = Node::Linear { symbol: "°", base: rad, scale: OrderedFloat(PI / 180.0) },
    /// Arc-minute
    arcmin = Node::Linear { symbol: "′", base: rad, scale: OrderedFloat(PI / 10800.0) },
    /// Arc-second
    arcsec = Node::Linear { symbol: "″", base: rad, scale: OrderedFloat(PI / 648000.0) },

    /* Energy */
    /// Electron-volt
    eV = Node::Linear { symbol: "eV", base: J, scale: OrderedFloat(1.602176634e-19) },
    /// (small) calorie
    cal = Node::Linear { symbol: "cal", base: J, scale: OrderedFloat(4.184) },

    /* Mass */

    /// Metric ton
    t = Node::Linear { symbol: "t", base: kg, scale: OrderedFloat(1000.0) },
    /// Atomic mass unit
    u = Node::Linear { symbol: "u", base: kg, scale: OrderedFloat(1.660_539_066_60e-27) },

    /* Astronomy */
    /// Astronomical Unit
    au = Node::Linear { symbol: "au", base: m, scale: OrderedFloat(149_597_870_700.0) },
    /// Light-year
    ly = Node::Linear { symbol: "ly", base: m, scale: OrderedFloat(9.460_730_472_580_8e15) },

    /* Pressure */

    /// Standard atmospheric pressure
    atm = Node::Linear { symbol: "atm", base: Pa, scale: OrderedFloat(101325.0) },
    /// Bar
    bar = Node::Linear { symbol: "bar", base: Pa, scale: OrderedFloat(1e5) },
    // Pound per square-inch
    psi = Node::Linear { symbol: "psi", base: Pa, scale: OrderedFloat(6894.757293168) },

    /* Temperature */

    /// Degree Celsius
    oC = Node::Affine { symbol: "°C", base: K, scale: OrderedFloat(1.0), offset: OrderedFloat(273.15) },

    /// Degree Farenheit
    oF = Node::Affine { symbol: "°F", base: K, scale: OrderedFloat(5. / 9.), offset: OrderedFloat(459.67 * 5. / 9.) },

    /// Degree Rankine
    oR = Node::Linear { symbol: "°Ra", base: K, scale: OrderedFloat(5. / 9.) },

    /* Logarthmic */
    /// Bel
    B = Node::Logarithmic { reference: UNITLESS, symbol: "B", base: OrderedFloat(10.0) },

    /// Neper
    Np = Node::Logarithmic { reference: UNITLESS, symbol: "B", base: OrderedFloat(E) }
}
