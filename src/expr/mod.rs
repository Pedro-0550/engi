use std::{
    array,
    fmt::{Debug, Display, Pointer},
    hash::Hash,
    mem::{discriminant, take},
    num::NonZero,
    rc::Rc,
    sync::Arc,
};

use dashmap::mapref::one::Ref;
use derive_more::{Deref, DerefMut, From, IsVariant};
use itertools::Itertools;

use crate::{
    core::{
        interned::{Handle, Interned},
        util::impl_as_variant,
    },
    expr::ops::{Binary, Matrix, Unary, Variadic},
    simplify::{Simplify, normal::Normalize},
    symbol::Symbol,
    units::Quantity,
};

/* -------------------------------- CONSTANTS ------------------------------- */

// static NODES: Arena<Node> = Arena::new();

/* --------------------------------- MODULES -------------------------------- */

pub mod impls;
pub mod ops;

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(PartialEq, Clone, From, IsVariant, Hash, Eq)]
#[from(forward)]
pub enum Node {
    Symbol(Symbol),
    Const(Quantity),
    Variadic(Variadic),
    Unary(Unary),
    Binary(Binary),
    Matrix(Matrix),
}

#[derive(Clone, Eq)]
pub struct Expr {
    node: Arc<Node>,
    hash: u64,
}

/* --------------------------------- STRUCTS -------------------------------- */

// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
// pub struct Expr(Handle<Node>);

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    from: Symbol,
    to: Expr,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Shape {
    rows: NonZero<usize>,
    cols: NonZero<usize>,
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Shaped {
    fn shape(&self) -> Shape;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(
    Node,
    [Symbol => Symbol,
    Const => Quantity,
    Variadic => Variadic,
    Unary => Unary,
    Binary => Binary,
    Matrix => Matrix,]
);

impl From<(usize, usize)> for Shape {
    fn from(value: (usize, usize)) -> Self {
        Self::rect(value.0, value.1)
    }
}

impl Shape {
    // SAFETY:
    // As of August 2026, 1 is not equal to 0.
    // If this changes in the future, use checked version instead.
    pub const SCALAR: Self = unsafe {
        Shape {
            cols: NonZero::<usize>::new_unchecked(1),
            rows: NonZero::<usize>::new_unchecked(1),
        }
    };

    pub fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    pub fn square(size: usize) -> Self {
        Self { rows: size.try_into().unwrap(), cols: size.try_into().unwrap() }
    }

    pub fn rect(rows: usize, cols: usize) -> Self {
        Self { rows: rows.try_into().unwrap(), cols: cols.try_into().unwrap() }
    }

    pub fn is_scalar(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() == 1
    }

    pub fn is_row(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() == 1
    }

    pub fn is_col(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() > 1
    }

    pub fn is_vec(&self) -> bool {
        (self.rows.get() > 1) ^ (self.cols.get() > 1)
    }

    pub fn is_rect(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() > 1
    }

    pub fn is_square(&self) -> bool {
        self.rows.get() > 1 && self.rows == self.rows
    }
}

impl Hash for Expr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl Expr {
    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        let Expr { node, .. } = self;

        match Arc::try_unwrap(node) {
            Ok(node) => node,
            Err(node) => (*node).clone(),
        }
    }

    /// Returns the total number of nodes in this expression
    pub fn size(&self) -> usize {
        match self.node() {
            Node::Symbol(_symbol) => 1,
            Node::Const(_quantity) => 1,
            Node::Variadic(variadic) => {
                variadic.operands().iter().map(|x| x.size()).sum::<usize>() + 1
            }
            Node::Unary(single) => single.arg().size() + 1,
            Node::Binary(double) => {
                double.args()[0].size() + double.args()[1].size() + 1
            }
            Node::Matrix(matrix) => {
                matrix.elements().iter().map(|x| x.size()).sum::<usize>() + 1
            }
        }
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        fn symbols_inner(expr: &Expr, vec: &mut Vec<Symbol>) {
            match expr.node() {
                Node::Symbol(symbol) => vec.push(*symbol),
                Node::Const(quantity) => (),
                Node::Variadic(variadic) => {
                    for op in variadic.operands() {
                        symbols_inner(op, vec);
                    }
                }
                Node::Unary(unary) => symbols_inner(unary.arg(), vec),
                Node::Binary(binary) => {
                    symbols_inner(binary.args()[0], vec);
                    symbols_inner(binary.args()[1], vec);
                }
                Node::Matrix(matrix) => {
                    for element in matrix.elements() {
                        symbols_inner(element, vec);
                    }
                }
            }
        }

        let mut vec = Vec::new();
        symbols_inner(self, &mut vec);
        vec
    }

    pub fn substitute(&self, bindings: &[Binding]) -> Self {
        match self.node() {
            Node::Variadic(op) => op
                .with_operands(
                    op.operands()
                        .iter()
                        .map(|x| x.substitute(bindings))
                        .collect(),
                )
                .into(),
            Node::Unary(op) => {
                op.with_arg(op.arg().substitute(bindings)).into()
            }
            Node::Const(qty) => qty.into(),

            Node::Binary(op) => op
                .with_args(array::from_fn(|i| {
                    op.args()[i].substitute(bindings)
                }))
                .into(),

            Node::Symbol(sym) => {
                if let Some(binding) = bindings.iter().find(|b| b.from == *sym)
                {
                    binding.to.clone()
                } else {
                    self.clone()
                }
            }

            Node::Matrix(m) => {
                Node::Matrix(m.map(|el| el.substitute(bindings))).into()
            }
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Shaped for Expr {
    fn shape(&self) -> Shape {
        match self.node() {
            Node::Symbol(symbol) => symbol.shape(),
            Node::Const(_) => Shape::SCALAR,
            Node::Variadic(variadic) => variadic.shape(),
            Node::Unary(single) => single.shape(),
            Node::Binary(double) => double.shape(),
            Node::Matrix(matrix) => matrix.shape(),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.node() {
            Node::Const(qty) => <Quantity as Display>::fmt(&qty, f),
            Node::Binary(op) => <Binary as Display>::fmt(&op, f),
            Node::Unary(op) => <Unary as Display>::fmt(&op, f),
            Node::Variadic(op) => <Variadic as Display>::fmt(&op, f),
            Node::Symbol(symb) => <Symbol as Display>::fmt(&symb, f),
            Node::Matrix(_m) => todo!(),
        }
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.node() {
            Node::Const(qty) => <Quantity as Display>::fmt(&qty, f),
            Node::Binary(op) => write!(f, "{:?}", op),
            Node::Unary(op) => write!(f, "{:?}", op),
            Node::Variadic(op) => write!(f, "{:?}", op),
            Node::Symbol(symb) => <Symbol as Display>::fmt(&symb, f),
            Node::Matrix(_m) => todo!(),
        }
    }
}
