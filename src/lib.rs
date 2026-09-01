#![feature(vec_try_remove)]
#![feature(const_trait_impl)]
#![feature(const_ops)]
#![feature(generic_atomic)]
#![feature(iter_map_windows)]
#![feature(const_try)]
#![feature(duration_constructors)]
#![feature(default_field_values)]
#![feature(associated_type_defaults)]
#![feature(macro_derive)]
#![feature(min_specialization)]

use num::Complex;

pub mod core {
    pub mod graph;
    pub mod interned;
    pub mod util;
    pub mod value;
}

pub mod diff;
pub mod expr;
pub mod simplify;
pub mod symbol;
pub mod system;
pub mod units;
