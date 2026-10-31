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
#![warn(unused_crate_dependencies)]

pub mod core {
    pub mod consed;
    pub mod index;
    pub mod interned;
    pub mod util;
    pub mod value;
}

pub mod diff;
pub mod expr;
pub mod model;
pub mod simplify;
pub mod symbol;
pub mod units;
