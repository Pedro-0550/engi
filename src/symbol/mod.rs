use std::{
    fmt::Display,
    hash::Hash,
    sync::{LazyLock, RwLock},
};

use bon::Builder;

use crate::{
    core::consed::{ConcurrentConsed, Consed},
    expr::{domain::Domain, shape::Shape},
    id,
    symbol::symbol_builder::IsComplete,
    unit::{Unit, defs::UNITLESS},
};

/* --------------------------------- MODULES -------------------------------- */

pub mod constants;

/* -------------------------------- CONSTANTS ------------------------------- */

static SYMBOLS: LazyLock<ConcurrentConsed<Meta, MetaId>> =
    LazyLock::new(|| ConcurrentConsed::default());
id!(MetaId);

/* --------------------------------- STRUCTS -------------------------------- */

// pub struct SymbolicContext {
//     info: HashMap<SymbolId, SymbolInfo>,
//     next_id: SymbolId,
// }

#[derive(Clone, Hash, Eq, PartialEq, Builder)]
#[builder(
    builder_type(
        name = SymbolBuilder,
        vis = "pub"
    ),
    finish_fn(
        name = build_internal,
        vis = "pub(self)",
    )
)]
struct Meta {
    pub name: &'static str,
    #[builder(default = "")]
    pub desc: &'static str,
    #[builder(default = UNITLESS)]
    pub unit: Unit,
    #[builder(default = Shape::SCALAR)]
    pub shape: Shape,
    #[builder(default = Domain::REAL)]
    pub domain: Domain,
    #[builder(default = Realization::Primary)]
    pub realization: Realization,
}

#[derive(PartialEq, Clone, Debug, Copy, Hash, Eq, PartialOrd, Ord)]
pub struct Symbol(pub(crate) MetaId);

#[macro_export]
macro_rules! symbols {
    ($($sym:ident),+) => {
        $(
            let $sym = engi::symbol::Symbol::new(stringify!($sym));
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

impl<S> SymbolBuilder<S>
where
    S: IsComplete,
{
    pub fn build(self) -> Symbol {
        let meta = self.build_internal();
        let id = SYMBOLS.insert(meta);

        Symbol(id)
    }
}

impl Symbol {
    pub fn builder() -> SymbolBuilder {
        Meta::builder()
    }

    pub fn new(name: &str) -> Self {
        let id = SYMBOLS.insert(Meta {
            name: name.to_owned().leak(),
            desc: "",
            unit: UNITLESS,
            shape: Shape::SCALAR,
            domain: Domain::COMPLEX,
            realization: Realization::Primary,
        });

        Symbol(id)
    }

    /// Splits off an imaginary part from this symbol. Returns itself if its already an imag part, or None if its a real part.
    pub fn imag(self) -> Option<Self> {
        match self.realization() {
            Realization::Primary => {
                let id = SYMBOLS.insert(Meta {
                    name: format!("Im{{{}}}", self.name()).leak(),
                    desc: format!("{} (imag part)", self.desc()).leak(),
                    unit: self.unit(),
                    shape: self.shape(),
                    domain: Domain::IMAG,
                    realization: Realization::Imag(self),
                });

                Some(Symbol(id))
            }
            Realization::Real(_symbol) => None,
            Realization::Imag(_symbol) => Some(self),
        }
    }

    /// Splits off a real part from this symbol. Returns itself if its already a real part, or None if its an imag part.
    pub fn real(self) -> Option<Self> {
        match self.realization() {
            Realization::Primary => {
                let id = SYMBOLS.insert(Meta {
                    name: format!("Re{{{}}}", self.name()).leak(),
                    desc: format!("{} (real part)", self.desc()).leak(),
                    unit: self.unit(),
                    shape: self.shape(),
                    domain: Domain::REAL,
                    realization: Realization::Real(self),
                });

                Some(Symbol(id))
            }
            Realization::Real(_symbol) => Some(self),
            Realization::Imag(_symbol) => None,
        }
    }

    /// If this symbol is a realization of another symbol, return that, otherwise None.
    pub fn as_primary(self) -> Option<Self> {
        match SYMBOLS.con_get(self.0).expect("invalid symbol id").realization {
            Realization::Primary => None,
            Realization::Real(symbol) | Realization::Imag(symbol) => {
                Some(symbol)
            }
        }
    }

    /* -------------------------------------------------------------------------- */

    pub fn realization(self) -> Realization {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").realization
    }

    pub fn name(&self) -> &str {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").name
    }

    pub fn unit(&self) -> Unit {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").unit.clone()
    }

    pub fn shape(&self) -> Shape {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").shape.clone()
    }

    pub fn desc(&self) -> &str {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").desc
    }

    pub fn domain(&self) -> Domain {
        SYMBOLS.con_get(self.0).expect("invalid symbol id").domain
    }
}

/* -------------------------------------------------------------------------- */

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
