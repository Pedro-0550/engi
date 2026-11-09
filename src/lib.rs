#![feature(const_trait_impl)]
#![feature(generic_atomic)]
#![feature(default_field_values)]
#![feature(min_specialization)]
#![feature(macro_metavar_expr)]
#![feature(try_blocks)]
#![feature(variant_count)]
#![feature(never_type)]
#![feature(const_convert)]
#![feature(option_zip)]
#![feature(mapped_lock_guards)]
#![feature(const_default)]
#![warn(unused_crate_dependencies)]
pub use tabled;

// pub mod math;
pub mod core {
    pub mod consed;
    pub mod indexed;
    pub mod util;
    pub mod value;
}

pub mod diff;
pub mod expr;
pub mod model;
pub mod simplify;
pub mod symbol;
pub mod units;

pub mod prelude {
    pub use num::pow::Pow;

    pub use crate::{
        model::{
            Interface, InterfaceArrayExt, Model, ModelBuilder, System,
            Variable, VariableBuilder,
        },
        units::{Quantity, defs::*, ops::*},
    };
}
