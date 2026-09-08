#![allow(non_upper_case_globals)]

use std::{fmt::Display, sync::LazyLock};

use crate::{
    core::value::Value,
    symbol::Symbol,
    units::{Quantity, Unit, si::*},
};

macro_rules! constants {
    (
        $(
            $(#[$meta:meta])*
            $name:ident = $value:expr
        ),* $(,)?
    ) => {
        $(
            $(#[$meta])*
            pub const $name: Constant = Constant {
                name: stringify!($name),
                value: || $value,
            };
        )*

        pub const CONSTANTS: [Constant; constants!(@count $($name),*)] = [
            $(
                $name,
            )*
        ];
    };

    (@count $($name:ident),*) => {
        <[()]>::len(&[$(constants!(@unit $name)),*])
    };

    (@unit $name:ident) => {
        ()
    };
}

/* -------------------------------- CONSTANTS ------------------------------- */

// pub const CONSTANTS: [Constant; 1] = [Constant {
//     name: "π",
//     desc: "Archimedes's constant",
//     value: LazyLock::new(|| 3.1415926535897932384 * Unit::Unitless),
// }];

// pub const π: Constant = CONSTANTS[1];

constants! {
    /// Archimedes's constant
    π = 3.1415926535897932384 * Unit::Unitless,

    /// Euler's number
    e = 2.7182818284590452353 * Unit::Unitless,

    /// Speed of light in a vacuum
    ///
    /// Source: CODATA 2022
    c = 299792458.0 * m / s,

    /// Boltzmann constant
    ///
    /// Source: CODATA 2022
    kB = 1.380649e-23 * J / K,


    /// Elementary charge
    ///
    /// Source: CODATA 2022
    q = 1.602176634e-19 * C,

    // TODO: add more useful constants
}

#[derive(Hash, Debug, PartialEq)]
pub struct Constant {
    name: &'static str,
    value: fn() -> Quantity,
}

impl Constant {
    pub fn quantity(&self) -> Quantity {
        (self.value)()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl Clone for Constant {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), value: self.value.clone() }
    }
}

impl Eq for Constant {}

impl Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}
