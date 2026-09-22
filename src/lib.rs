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
#![feature(iterator_try_reduce)]
#![feature(iter_array_chunks)]
#![feature(derive_const)]
#![feature(const_default)]
#![feature(box_patterns)]

pub mod core {
    pub mod graph;
    pub mod interned;
    pub mod util;
    pub mod value;
}

pub mod diff;
pub mod expr;
pub mod model;
// pub mod simplify;
pub mod normal;
pub mod symbol;
pub mod units;
