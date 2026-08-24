use std::{
    fmt::Display,
    hash::Hash,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    core::arena::{Arena, Handle},
    dimension::Unit,
    expr::Shape,
    // set::Set,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod constants;

/* -------------------------------- CONSTANTS ------------------------------- */

static SYMBOLS: Arena<SymbolInfo> = Arena::new();
static CONSTANTS_REGISTERED: AtomicBool = AtomicBool::new(false);

/* --------------------------------- STRUCTS -------------------------------- */

// pub struct SymbolicContext {
//     info: HashMap<SymbolId, SymbolInfo>,
//     next_id: SymbolId,
// }

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct SymbolInfo {
    name: String,
    description: String,
    unit: Unit,
    shape: Shape, // domain: Set,
}

#[derive(PartialEq, Clone, Debug, Copy, Hash, Eq)]
pub struct Symbol(pub(crate) Handle<SymbolInfo>);

#[macro_export]
macro_rules! symbols {
    ($($sym:ident),+) => {
        $(
            let $sym = Symbol::new(stringify!($sym));
        )+
    };
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Symbol {
    pub fn new(name: &str) -> Self {
        if !CONSTANTS_REGISTERED.load(Ordering::SeqCst) {
            constants::register();
            CONSTANTS_REGISTERED.store(true, Ordering::SeqCst);
        }

        let handle = SYMBOLS.insert(SymbolInfo {
            name: name.to_owned(),
            description: String::new(),
            unit: Unit::Unitless,
            shape: Shape::SCALAR, // domain: Set::C,
        });

        Symbol(handle)
    }

    // pub fn set_domain(self, domain: Set) -> Self {
    //     SYMBOLS.modify(self.0, |i| i.domain = domain);
    //     self
    // }

    pub fn name(&self) -> String {
        SYMBOLS.get_cloned(self.0).expect("invalid symbol handle").name
    }

    pub fn unit(&self) -> Unit {
        SYMBOLS.get_cloned(self.0).expect("invalid symbol handle").unit
    }

    pub fn set_unit(self, unit: Unit) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.unit = unit);
        self
    }

    pub fn shape(&self) -> Shape {
        SYMBOLS.get_cloned(self.0).expect("invalid symbol handle").shape
    }

    pub fn set_shape(self, shape: Shape) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.shape = shape);
        self
    }

    pub fn description(&self) -> String {
        SYMBOLS.get_cloned(self.0).expect("invalid symbol handle").description
    }

    pub fn set_description(self, description: String) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.description = description);
        self
    }

    // pub fn domain(&self) -> Set {
    //     SYMBOLS.get_cloned(self.0).expect("invalid symbol handle").domain
    // }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}
