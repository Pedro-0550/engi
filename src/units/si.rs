#![allow(non_upper_case_globals)]

use crate::units::{Unit, isq::*};

/* -------------------------------------------------------------------------- */

pub const s: Unit = Unit::Base { dimension: TIME, symbol: "s" };

pub const m: Unit = Unit::Base { dimension: LENGTH, symbol: "m" };

pub const kg: Unit = Unit::Base { dimension: MASS, symbol: "kg" };

pub const A: Unit = Unit::Base { dimension: ELECTRIC_CURRENT, symbol: "A" };

pub const K: Unit = Unit::Base { dimension: TEMPERATURE, symbol: "K" };

pub const cd: Unit = Unit::Base { dimension: LUMINOUS_INTENSITY, symbol: "cd" };

pub const mol: Unit =
    Unit::Base { dimension: AMOUNT_OF_SUBSTANCE, symbol: "mol" };

pub const rad: Unit = Unit::Base { dimension: DIMENSIONLESS, symbol: "rad" };

pub const sr: Unit = Unit::Base { dimension: DIMENSIONLESS, symbol: "sr" };

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

pub const Ω: Unit =
    Unit::Derived { symbol: "Ω", base: &[(kg, 1), (m, 2), (s, -3), (A, -2)] };

pub const S: Unit =
    Unit::Derived { symbol: "S", base: &[(kg, -1), (m, -2), (s, 3), (A, 2)] };

pub const F: Unit =
    Unit::Derived { symbol: "F", base: &[(kg, -1), (m, -2), (s, 4), (A, 2)] };

pub const Wb: Unit =
    Unit::Derived { symbol: "Wb", base: &[(kg, 1), (m, 2), (s, -2), (A, -1)] };

pub const T: Unit =
    Unit::Derived { symbol: "T", base: &[(kg, 1), (s, -2), (A, -1)] };

pub const H: Unit =
    Unit::Derived { symbol: "H", base: &[(kg, 1), (m, 2), (s, -2), (A, -2)] };
