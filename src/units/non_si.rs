#![allow(non_upper_case_globals)]
use std::f64::consts::PI;

use ordered_float::OrderedFloat;

use crate::units::{Unit, si::*};

// Time
//
/// Minute
pub const min: Unit =
    Unit::Scaled { symbol: "min", base: &s, scale: OrderedFloat(60.0) };

/// Hour
pub const hr: Unit =
    Unit::Scaled { symbol: "hr", base: &s, scale: OrderedFloat(3600.0) };

pub const day: Unit =
    Unit::Scaled { symbol: "d", base: &s, scale: OrderedFloat(86400.0) };

// Length

/// Inches
pub const inch: Unit =
    Unit::Scaled { symbol: "in", base: &m, scale: OrderedFloat(0.0254) };

/// Feet
pub const ft: Unit =
    Unit::Scaled { symbol: "ft", base: &m, scale: OrderedFloat(0.3048) };

/// Imperial yard
pub const yd: Unit =
    Unit::Scaled { symbol: "yd", base: &m, scale: OrderedFloat(0.9144) };

/// Imperial miles
pub const mi: Unit =
    Unit::Scaled { symbol: "mi", base: &m, scale: OrderedFloat(1609.344) };

/// Nautical miles
pub const nmi: Unit =
    Unit::Scaled { symbol: "nmi", base: &m, scale: OrderedFloat(1852.0) };

/// Degree
pub const deg: Unit =
    Unit::Scaled { symbol: "°", base: &rad, scale: OrderedFloat(PI / 180.0) };

/// Arc-minute, 1/60th of a degree
pub const arcmin: Unit =
    Unit::Scaled {
        symbol: "′", base: &rad, scale: OrderedFloat(PI / 10800.0)
    };

/// Arc-second, 1/3600th of a degree
pub const arcsec: Unit = Unit::Scaled {
    symbol: "″",
    base: &rad,
    scale: OrderedFloat(PI / 648000.0),
};

// Energy

/// Electron-volt
pub const eV: Unit = Unit::Scaled {
    symbol: "eV",
    base: &J,
    scale: OrderedFloat(1.602176634e-19),
};

/// Small calorie
pub const cal: Unit =
    Unit::Scaled { symbol: "cal", base: &J, scale: OrderedFloat(4.184) };

// Mass

/// Metric Ton
pub const t: Unit =
    Unit::Scaled { symbol: "t", base: &kg, scale: OrderedFloat(1000.0) };

/// Atomic mass unit
pub const u: Unit = Unit::Scaled {
    symbol: "u",
    base: &kg,
    scale: OrderedFloat(1.660_539_066_60e-27),
};

/// Astronomical unit
pub const au: Unit = Unit::Scaled {
    symbol: "au",
    base: &m,
    scale: OrderedFloat(149_597_870_700.0),
};

/// Light-year
pub const ly: Unit = Unit::Scaled {
    symbol: "ly",
    base: &m,
    scale: OrderedFloat(9.460_730_472_580_8e15),
};

// Pressure

/// Standard atmospheric pressure
pub const atm: Unit =
    Unit::Scaled { symbol: "atm", base: &Pa, scale: OrderedFloat(9.86923e6) };

/// Bar
pub const bar: Unit =
    Unit::Scaled { symbol: "bar", base: &Pa, scale: OrderedFloat(1e5) };

/// Pounds per square inch
pub const psi: Unit =
    Unit::Scaled { symbol: "psi", base: &Pa, scale: OrderedFloat(1.45038e4) };

// Volume
