#![allow(non_upper_case_globals)]

use crate::{
    symbol::Symbol,
    units::{Unit, si::*},
};

macro_rules! constants {
    (
        $(
            $(#[$meta:meta])*
            $name:ident = $value:expr
        ),* $(,)?
    ) => {
        constants!(@defs 0;
            $(
                $(#[$meta])*
                $name = $value
            ),*
        );

        pub(super) fn register() {
            constants!(@register 0;
                $(
                    $name = $value
                ),*
            );
        }
    };

    (@defs $i:expr;
        $(#[$meta:meta])*
        $name:ident = $value:expr
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub const $name: Symbol =
            Symbol(crate::core::interned::Handle::new($i));

        constants!(@defs $i + 1; $($($rest)*)?);
    };

    (@defs $i:expr;) => {};

    (@register $i:expr;
        $name:ident = $value:expr
        $(, $($rest:tt)*)?
    ) => {
        crate::symbol::SYMBOLS.insert_at(
            $i,
            crate::symbol::SymbolInfo {
                name: stringify!($name).to_owned(),
                unit: ($value).unit(),
                shape: crate::expr::Shape::SCALAR,
                desc: String::new(),
                // domain: crate::set::Set::C_NZ
            },
        );

        constants!(@register $i + 1; $($($rest)*)?);
    };

    (@register $i:expr;) => {};
}

/* -------------------------------- CONSTANTS ------------------------------- */

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
