use std::{
    array,
    cell::{LazyCell, OnceCell},
    collections::HashMap,
    fmt::{Debug, Display, Pointer},
    hash::{BuildHasher, Hash, Hasher},
    mem::{discriminant, take},
    num::NonZero,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{
            AbiParam, FuncRef, InstBuilder, MemFlags, MemFlagsData,
            UserFuncName,
            types::{self, F64},
        },
        settings::{self, Configurable, Flags},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext},
    jit::{JITBuilder, JITModule},
    module::{Linkage, Module, default_libcall_names},
};
use dashmap::mapref::one::Ref;
use derive_more::{Deref, DerefMut, From, IsVariant};
use itertools::Itertools;
use num::complex::{Complex64, ComplexFloat};
use ordered_float::Pow;

use crate::{
    core::{
        interned::{Handle, Interned},
        util::impl_as_variant,
        value::Value,
    },
    expr::mat::Matrix,
    simplify::{Simplify, normal::Normalize},
    symbol::{
        Symbol,
        constants::{Constant, e, π},
    },
    units::Quantity,
};

/* --------------------------------- MODULES -------------------------------- */

pub mod domain;
pub mod fmt;
pub mod jit;
pub mod mat;
pub mod ops;
pub mod shape;

/* --------------------------------- ALIASES -------------------------------- */

type Bindings = HashMap<Symbol, Expr>;

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Eq, Clone, PartialEq, Hash, Debug, From, IsVariant)]
pub enum Node {
    #[from]
    Symbol(Symbol),
    #[from]
    Constant(Constant),
    #[from]
    Quantity(Quantity),

    Add(Box<[Expr]>),
    Mul(Box<[Expr]>),
    Min(Box<[Expr]>),
    Max(Box<[Expr]>),

    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),

    Asin(Box<Expr>),
    Acos(Box<Expr>),
    Atan(Box<Expr>),

    Sinh(Box<Expr>),
    Cosh(Box<Expr>),
    Tanh(Box<Expr>),

    Asinh(Box<Expr>),
    Acosh(Box<Expr>),
    Atanh(Box<Expr>),

    Arg(Box<Expr>),
    Conj(Box<Expr>),
    Norm(Box<Expr>),
    Sign(Box<Expr>),

    Real(Box<Expr>),
    Imag(Box<Expr>),

    Pow {
        base: Box<Expr>,
        exp: Box<Expr>,
    },
    Log {
        base: Box<Expr>,
        arg: Box<Expr>,
    },
    Atan2 {
        a: Box<Expr>,
        b: Box<Expr>,
    },

    #[from]
    Matrix(Matrix),
    Transpose(Box<Expr>),
    Det(Box<Expr>),
    Rank(Box<Expr>),
    Trace(Box<Expr>),

    Piecewise {
        cond: Box<Condition>,
        pass: Box<Expr>,
        fail: Box<Expr>,
    },
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum Condition {
    Eq(Expr, Expr),
    Ne(Expr, Expr),
    Lt(Expr, Expr),
    Le(Expr, Expr),
    Gt(Expr, Expr),
    Ge(Expr, Expr),

    And(Box<[Condition]>),
    Or(Box<[Condition]>),
    Not(Box<Condition>),
}

#[derive(Clone, Eq)]
pub struct Expr {
    node: Node,
    key: OnceCell<u64>,
}

/* --------------------------------- STRUCTS -------------------------------- */

// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
// pub struct Expr(Handle<Node>);

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(Node, [Quantity => Quantity, Constant => Constant, Symbol => Symbol]);

impl Hash for Expr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.key());
    }
}

impl Expr {
    /// Returns an iterator over the immediate children of this expr node.
    pub fn iter_children<'s>(&'s self) -> Box<dyn Iterator<Item = &Expr> + 's> {
        match &self.node {
            Node::Symbol(_) | Node::Constant(_) | Node::Quantity(_) => {
                Box::new(std::iter::empty())
            }
            Node::Add(nodes)
            | Node::Mul(nodes)
            | Node::Min(nodes)
            | Node::Max(nodes) => Box::new(nodes.iter()),
            Node::Sin(node)
            | Node::Cos(node)
            | Node::Tan(node)
            | Node::Asin(node)
            | Node::Acos(node)
            | Node::Atan(node)
            | Node::Sinh(node)
            | Node::Cosh(node)
            | Node::Tanh(node)
            | Node::Asinh(node)
            | Node::Acosh(node)
            | Node::Atanh(node)
            | Node::Arg(node)
            | Node::Conj(node)
            | Node::Norm(node)
            | Node::Sign(node)
            | Node::Real(node)
            | Node::Imag(node)
            | Node::Det(node)
            | Node::Transpose(node)
            | Node::Rank(node)
            | Node::Trace(node) => Box::new(std::iter::once(node.as_ref())),
            Node::Pow { base, exp } => {
                Box::new([base.as_ref(), exp.as_ref()].into_iter())
            }
            Node::Log { base, arg } => {
                Box::new([base.as_ref(), arg.as_ref()].into_iter())
            }
            Node::Atan2 { a, b } => {
                Box::new([a.as_ref(), b.as_ref()].into_iter())
            }
            Node::Matrix(matrix) => Box::new(matrix.elements().iter()),
            Node::Piecewise { cond, pass, fail } => todo!(),
        }
    }

    pub fn key(&self) -> u64 {
        *self.key.get_or_init(|| {
            let mut hasher = ahash::RandomState::with_seed(1).build_hasher();
            self.node.hash(&mut hasher);
            hasher.finish()
        })
    }

    /// Returns an iterator over the immediate children of this expr node.
    pub fn into_iter_children(self) -> Box<dyn Iterator<Item = Expr>> {
        match self.node {
            Node::Symbol(_) | Node::Constant(_) | Node::Quantity(_) => {
                Box::new(std::iter::empty())
            }
            Node::Add(nodes)
            | Node::Mul(nodes)
            | Node::Min(nodes)
            | Node::Max(nodes) => Box::new(nodes.into_iter()),
            Node::Sin(node)
            | Node::Cos(node)
            | Node::Tan(node)
            | Node::Asin(node)
            | Node::Acos(node)
            | Node::Atan(node)
            | Node::Sinh(node)
            | Node::Cosh(node)
            | Node::Tanh(node)
            | Node::Asinh(node)
            | Node::Acosh(node)
            | Node::Atanh(node)
            | Node::Arg(node)
            | Node::Conj(node)
            | Node::Norm(node)
            | Node::Sign(node)
            | Node::Real(node)
            | Node::Imag(node)
            | Node::Det(node)
            | Node::Transpose(node)
            | Node::Rank(node)
            | Node::Trace(node) => Box::new(std::iter::once(*node)),
            Node::Pow { base, exp } => Box::new([*base, *exp].into_iter()),
            Node::Log { base, arg } => Box::new([*base, *arg].into_iter()),
            Node::Atan2 { a, b } => Box::new([*a, *b].into_iter()),
            Node::Matrix(matrix) => {
                Box::new(matrix.into_elements().into_iter())
            }
            Node::Piecewise { cond, pass, fail } => todo!(),
        }
    }

    /// Applys a function to every child of this node, and returns the same node kind with the new children.
    pub fn map_children(&self, f: impl FnMut(Expr) -> Expr) -> Expr {
        let shape = self.shape();
        let mut mapped = self.iter_children().cloned().map(f);

        let node = match self.node {
            Node::Symbol(_) => self.node.clone(),
            Node::Constant(_) => self.node.clone(),
            Node::Quantity(_) => self.node.clone(),

            Node::Add(_) => Node::Add(mapped.collect()),
            Node::Mul(_) => Node::Mul(mapped.collect()),
            Node::Min(_) => Node::Min(mapped.collect()),
            Node::Max(_) => Node::Max(mapped.collect()),

            Node::Sin(_) => Node::Sin(Box::new(mapped.next().unwrap())),
            Node::Cos(_) => Node::Cos(Box::new(mapped.next().unwrap())),
            Node::Tan(_) => Node::Tan(Box::new(mapped.next().unwrap())),

            Node::Asin(_) => Node::Asin(Box::new(mapped.next().unwrap())),
            Node::Acos(_) => Node::Acos(Box::new(mapped.next().unwrap())),
            Node::Atan(_) => Node::Atan(Box::new(mapped.next().unwrap())),

            Node::Sinh(_) => Node::Sinh(Box::new(mapped.next().unwrap())),
            Node::Cosh(_) => Node::Cosh(Box::new(mapped.next().unwrap())),
            Node::Tanh(_) => Node::Tanh(Box::new(mapped.next().unwrap())),

            Node::Asinh(_) => Node::Asinh(Box::new(mapped.next().unwrap())),
            Node::Acosh(_) => Node::Acosh(Box::new(mapped.next().unwrap())),
            Node::Atanh(_) => Node::Atanh(Box::new(mapped.next().unwrap())),

            Node::Arg(_) => Node::Arg(Box::new(mapped.next().unwrap())),
            Node::Conj(_) => Node::Conj(Box::new(mapped.next().unwrap())),
            Node::Norm(_) => Node::Norm(Box::new(mapped.next().unwrap())),
            Node::Sign(_) => Node::Sign(Box::new(mapped.next().unwrap())),

            Node::Real(_) => Node::Real(Box::new(mapped.next().unwrap())),
            Node::Imag(_) => Node::Imag(Box::new(mapped.next().unwrap())),

            Node::Pow { .. } => Node::Pow {
                base: Box::new(mapped.next().unwrap()),
                exp: Box::new(mapped.next().unwrap()),
            },

            Node::Log { .. } => Node::Log {
                base: Box::new(mapped.next().unwrap()),
                arg: Box::new(mapped.next().unwrap()),
            },

            Node::Atan2 { .. } => Node::Atan2 {
                a: Box::new(mapped.next().unwrap()),
                b: Box::new(mapped.next().unwrap()),
            },

            Node::Transpose(_) => {
                Node::Transpose(Box::new(mapped.next().unwrap()))
            }
            Node::Det(_) => Node::Det(Box::new(mapped.next().unwrap())),
            Node::Rank(_) => Node::Rank(Box::new(mapped.next().unwrap())),
            Node::Trace(_) => Node::Trace(Box::new(mapped.next().unwrap())),

            Node::Matrix(_) => {
                Node::Matrix(Matrix::from_elements(shape, mapped.collect()))
            }
            Node::Piecewise { .. } => todo!(),
        };

        node.into()
    }
}

impl Expr {
    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    /// Returns the total number of nodes in this expression
    pub fn size(&self) -> usize {
        1 + self.iter_children().map(Expr::size).sum::<usize>()
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        fn symbols_inner(expr: &Expr, vec: &mut Vec<Symbol>) {
            match expr.node() {
                Node::Symbol(symbol) => vec.push(*symbol),
                Node::Quantity(quantity) => (),
                Node::Constant(_) => (),
                _ => expr.iter_children().for_each(|s| symbols_inner(s, vec)),
            }
        }

        let mut vec = Vec::new();
        symbols_inner(self, &mut vec);
        vec.sort();
        vec.dedup();
        vec
    }

    pub fn substitute(&self, bindings: &Bindings) -> Self {
        match self.node() {
            Node::Constant(c) => c.into(),
            Node::Quantity(qty) => qty.into(),

            Node::Symbol(sym) => {
                if let Some(binding) = bindings.get(&sym) {
                    binding.clone()
                } else {
                    self.clone()
                }
            }

            _ => self.map_children(|c| c.substitute(bindings)),
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

macro_rules! impl_unary_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let expr = x.into();

            Node::$variant(Box::new(expr)).into()
        }
    };

    ($fn:ident, $variant:ident, $name:literal, square) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let expr = x.into();
            let shape = expr.shape();

            assert!(
                shape.is_square_mat() || shape.is_scalar(),
                "Matrix-valued {} is only defined for square matrices",
                $name
            );

            Node::$variant(Box::new(expr)).into()
        }
    };
}

impl_unary_fn!(sin, Sin, "sine", square);
impl_unary_fn!(cos, Cos, "cosine", square);
impl_unary_fn!(tan, Tan, "tangent", square);

impl_unary_fn!(asin, Asin, "inverse sine", square);
impl_unary_fn!(acos, Acos, "inverse cosine", square);
impl_unary_fn!(atan, Atan, "inverse tangent", square);

impl_unary_fn!(sinh, Sinh, "hyperbolic sine", square);
impl_unary_fn!(cosh, Cosh, "hyperbolic cosine", square);
impl_unary_fn!(tanh, Tanh, "hyperbolic tangent", square);

impl_unary_fn!(asinh, Asinh, "inverse hyperbolic sine", square);
impl_unary_fn!(acosh, Acosh, "inverse hyperbolic cosine", square);
impl_unary_fn!(atanh, Atanh, "inverse hyperbolic tangent", square);

impl_unary_fn!(real, Real, "real component of z");
impl_unary_fn!(imag, Imag, "imaginary component of z");
impl_unary_fn!(
    sign,
    Sign,
    "sign(z) = sign(real(z)) + i * sign(imag(z)), for a complex number z"
);

impl_unary_fn!(
    norm,
    Norm,
    "Norm of the given number, or Frobenius norm for matrices"
);

pub fn atan2(a: impl Into<Expr>, b: impl Into<Expr>) -> Expr {
    let a = Box::new(a.into());
    let b = Box::new(b.into());

    assert!(
        a.shape().is_scalar() && b.shape().is_scalar(),
        "atan2 is only defined for scalars"
    );

    Node::Atan2 { a, b }.into()
}

pub fn log(base: impl Into<Expr>, x: impl Into<Expr>) -> Expr {
    let base = Box::new(base.into());
    let x = Box::new(x.into());

    assert!(
        base.shape().is_scalar(),
        "Logarithm is only defined for scalar bases"
    );

    assert!(
        x.shape().is_square_mat() || x.shape().is_scalar(),
        "Matrix-valued logarithm is only defined for square matrices"
    );

    Node::Log { base, arg: x }.into()
}

pub fn ln(x: impl Into<Expr>) -> Expr {
    log(e, x)
}

pub fn exp(x: impl Into<Expr>) -> Expr {
    e.pow(x.into())
}

pub fn sqrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 2)
}

pub fn cbrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 3)
}

pub fn qtrt(x: impl Into<Expr>) -> Expr {
    x.into().pow(1 / 4)
}
