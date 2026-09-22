use std::{
    fmt::Display,
    hash::Hash,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};

use num::complex::Complex64;

use crate::{
    core::interned::{Handle, Interned},
    expr::{domain::Domain, shape::Shape},
    units::{Quantity, Unit},
};

/* --------------------------------- MODULES -------------------------------- */

pub mod constants;

/* -------------------------------- CONSTANTS ------------------------------- */

static SYMBOLS: Interned<SymbolInfo> = Interned::new();

/* --------------------------------- STRUCTS -------------------------------- */

// pub struct SymbolicContext {
//     info: HashMap<SymbolId, SymbolInfo>,
//     next_id: SymbolId,
// }

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct SymbolInfo {
    name: &'static str,
    desc: &'static str,
    unit: Unit,
    shape: Shape,
    domain: Domain,
    realization: Realization,
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

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Realization {
    /// Symbol is not realized
    Primary,
    /// Symbol is real part of another symbol
    Real(Symbol),
    /// Symbol is imaginary part of another symbol
    Imag(Symbol),
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Symbol {
    pub fn new(name: &str) -> Self {
        let handle = SYMBOLS.insert(SymbolInfo {
            name: name.to_owned().leak(),
            desc: "",
            unit: Unit::Unitless,
            shape: Shape::SCALAR,
            domain: Domain::COMPLEX,
            realization: Realization::Primary,
        });

        Symbol(handle)
    }

    /// Splits off an imaginary part from this symbol
    pub fn imag(self) -> Option<Self> {
        match self.realization() {
            Realization::Primary => {
                let handle = SYMBOLS.insert(SymbolInfo {
                    name: format!("Im{{{}}}", self.name()).leak(),
                    desc: format!("{} (imag part)", self.desc()).leak(),
                    unit: self.unit(),
                    shape: self.shape(),
                    domain: Domain::IMAG,
                    realization: Realization::Imag(self),
                });

                Some(Symbol(handle))
            }
            Realization::Real(symbol) => None,
            Realization::Imag(symbol) => Some(self),
        }
    }

    /// Splits off a real part from this symbol
    pub fn real(self) -> Option<Self> {
        match self.realization() {
            Realization::Primary => {
                let handle = SYMBOLS.insert(SymbolInfo {
                    name: format!("Re{{{}}}", self.name()).leak(),
                    desc: format!("{} (real part)", self.desc()).leak(),
                    unit: self.unit(),
                    shape: self.shape(),
                    domain: Domain::REAL,
                    realization: Realization::Real(self),
                });

                Some(Symbol(handle))
            }
            Realization::Real(symbol) => Some(self),
            Realization::Imag(symbol) => None,
        }
    }

    /// If this symbol is a realization of another symbol, return that, otherwise None.
    pub fn as_primary(self) -> Option<Self> {
        match SYMBOLS.get(self.0).expect("invalid symbol handle").realization {
            Realization::Primary => None,
            Realization::Real(symbol) | Realization::Imag(symbol) => {
                Some(symbol)
            }
        }
    }

    pub fn realization(self) -> Realization {
        SYMBOLS.get(self.0).expect("invalid symbol handle").realization
    }

    pub fn set_domain(self, domain: Domain) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.domain = domain);
        self
    }

    pub fn name(&self) -> &str {
        SYMBOLS.get(self.0).expect("invalid symbol handle").name
    }

    pub fn unit(&self) -> Unit {
        SYMBOLS.get(self.0).expect("invalid symbol handle").unit.clone()
    }

    pub fn set_unit(self, unit: Unit) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.unit = unit);
        self
    }

    pub fn shape(&self) -> Shape {
        SYMBOLS.get(self.0).expect("invalid symbol handle").shape.clone()
    }

    pub fn set_shape(self, shape: Shape) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.shape = shape);
        self
    }

    pub fn desc(&self) -> &str {
        SYMBOLS.get(self.0).expect("invalid symbol handle").desc
    }

    pub fn set_desc(self, desc: String) -> Self {
        SYMBOLS.modify(self.0, |mut i| i.desc = desc.leak());
        self
    }

    pub fn domain(&self) -> Domain {
        SYMBOLS.get(self.0).expect("invalid symbol handle").domain.clone()
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name().cmp(&other.name()).then_with(|| self.0.0.cmp(&other.0.0))
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
