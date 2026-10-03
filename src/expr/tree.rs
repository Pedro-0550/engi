use std::{
    hash::{Hash, Hasher},
    num::NonZero,
    ops::{Index, IndexMut},
};

use derive_more::IsVariant;
use kinded::Kinded;
use num::complex::Complex64;

use crate::{
    core::{util::impl_as_variant, value::Value},
    expr::shape::Shape,
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    symbol::{Symbol, constants::Constant},
    units::{Quantity, Unit::Unitless},
};

#[derive(Eq, PartialEq, Clone, Hash, Kinded, IsVariant)]
#[kinded(derive(Hash))]
pub enum Branch<N> {
    Add([N; 2]),
    Mul([N; 2]),
    Min([N; 2]),
    Max([N; 2]),

    Sin(N),
    Cos(N),
    Tan(N),

    Asin(N),
    Acos(N),
    Atan(N),

    Sinh(N),
    Cosh(N),
    Tanh(N),

    Asinh(N),
    Acosh(N),
    Atanh(N),

    Arg(N),
    Conj(N),
    Norm(N),
    Sign(N),

    Real(N),
    Imag(N),

    Pow { base: N, exp: N },
    Log { base: N, arg: N },
    Atan2 { a: N, b: N },

    Matrix(Matrix<N>),
    Transpose(N),
    Det(N),
    Rank(N),
    Trace(N),

    Conditional { cond: Condition<N>, pass: N, fail: N },
}

#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Matrix<N> {
    shape: Shape,
    elements: Box<[N]>,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum Condition<N> {
    Eq(N, N),
    Ne(N, N),
    Lt(N, N),
    Le(N, N),
    Gt(N, N),
    Ge(N, N),

    And(Box<[Condition<N>]>),
    Or(Box<[Condition<N>]>),
    Not(Box<Condition<N>>),
}

#[derive(PartialEq, Clone, Eq, Hash, Kinded, IsVariant)]
#[kinded(derive(Hash))]
pub enum Leaf {
    Symbol(Symbol),
    Constant(Constant),
    Quantity(Quantity),
}

#[derive(Eq, PartialEq, Clone, Hash, IsVariant)]
pub enum Node<N> {
    Leaf(Leaf),
    Branch(Branch<N>),
}

#[derive(PartialEq, Clone, Eq, Hash, Copy)]
pub enum NodeKind {
    Leaf(LeafKind),
    Branch(BranchKind),
}

/* -------------------------------------------------------------------------- */

impl_as_variant!(Leaf, [Symbol => Symbol, Quantity => Quantity, Constant => Constant]);

impl<N> Node<N> {
    pub fn as_leaf(&self) -> Option<&Leaf> {
        match self {
            Node::Leaf(leaf) => Some(leaf),
            Node::Branch(branch) => None,
        }
    }

    pub fn as_branch(&self) -> Option<&Branch<N>> {
        match self {
            Node::Branch(branch) => Some(branch),
            Node::Leaf(leaf) => None,
        }
    }

    pub fn as_branch_mut(&mut self) -> Option<&mut Branch<N>> {
        match self {
            Node::Branch(branch) => Some(branch),
            Node::Leaf(leaf) => None,
        }
    }

    pub fn kind(&self) -> NodeKind {
        match self {
            Node::Leaf(leaf) => NodeKind::Leaf(leaf.kind()),
            Node::Branch(branch) => NodeKind::Branch(branch.kind()),
        }
    }

    pub fn children(&self) -> Box<[&N]> {
        match self {
            Node::Leaf(_) => Box::new([]),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns.each_ref()),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new([n]),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp])
                }

                Branch::Atan2 { a, b } => Box::new([a, b]),

                Branch::Matrix(matrix) => matrix.elements().iter().collect(),

                Branch::Conditional { cond, pass, fail } => {
                    let mut out = Vec::new();
                    cond.push_children(&mut out);
                    out.push(pass);
                    out.push(fail);
                    out.into_boxed_slice()
                }
            },
        }
    }

    pub fn children_mut(&mut self) -> Box<[&mut N]> {
        match self {
            Node::Leaf(_) => Box::new([]),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns.each_mut()),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new([n]),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp])
                }

                Branch::Atan2 { a, b } => Box::new([a, b]),

                Branch::Matrix(matrix) => {
                    matrix.elements_mut().iter_mut().collect()
                }

                Branch::Conditional { cond, pass, fail } => {
                    let mut out = Vec::new();
                    cond.push_children_mut(&mut out);
                    out.push(pass);
                    out.push(fail);
                    out.into_boxed_slice()
                }
            },
        }
    }

    /// Cost function based on a cycle count heuristic on modern CPUs
    pub fn cost(&self) -> u8 {
        match self {
            Node::Leaf(leaf) => 1,
            Node::Branch(branch) => match branch {
                Branch::Add(_) => 1,
                Branch::Mul(_) => 1,
                Branch::Min(_) => 1,
                Branch::Max(_) => 1,
                Branch::Sin(_) => 2,
                Branch::Cos(_) => 2,
                Branch::Tan(_) => 2,
                Branch::Asin(_) => 3,
                Branch::Acos(_) => 3,
                Branch::Atan(_) => 3,
                Branch::Sinh(_) => 3,
                Branch::Cosh(_) => 3,
                Branch::Tanh(_) => 3,
                Branch::Asinh(_) => 3,
                Branch::Acosh(_) => 3,
                Branch::Atanh(_) => 3,
                Branch::Arg(_) => 3,
                Branch::Conj(_) => 1,
                Branch::Norm(_) => 3,
                Branch::Sign(_) => 1,
                Branch::Real(_) => 1,
                Branch::Imag(_) => 1,
                Branch::Pow { base, exp } => 4,
                Branch::Log { base, arg } => 4,
                Branch::Atan2 { a, b } => 3,
                Branch::Matrix(matrix) => todo!(),
                Branch::Transpose(_) => todo!(),
                Branch::Det(_) => todo!(),
                Branch::Rank(_) => todo!(),
                Branch::Trace(_) => todo!(),
                Branch::Conditional { cond, pass, fail } => todo!(),
            },
        }
    }

    pub fn into_children(self) -> Box<[N]> {
        match self {
            Node::Leaf(_) => Box::new([]),

            Node::Branch(branch) => match branch {
                Branch::Add(ns)
                | Branch::Mul(ns)
                | Branch::Min(ns)
                | Branch::Max(ns) => Box::new(ns),

                Branch::Sin(n)
                | Branch::Cos(n)
                | Branch::Tan(n)
                | Branch::Asin(n)
                | Branch::Acos(n)
                | Branch::Atan(n)
                | Branch::Sinh(n)
                | Branch::Cosh(n)
                | Branch::Tanh(n)
                | Branch::Asinh(n)
                | Branch::Acosh(n)
                | Branch::Atanh(n)
                | Branch::Arg(n)
                | Branch::Conj(n)
                | Branch::Norm(n)
                | Branch::Sign(n)
                | Branch::Real(n)
                | Branch::Imag(n)
                | Branch::Transpose(n)
                | Branch::Det(n)
                | Branch::Rank(n)
                | Branch::Trace(n) => Box::new([n]),

                Branch::Pow { base, exp } | Branch::Log { base, arg: exp } => {
                    Box::new([base, exp])
                }

                Branch::Atan2 { a, b } => Box::new([a, b]),

                Branch::Matrix(matrix) => matrix.into_elements(),

                Branch::Conditional { cond, pass, fail } => {
                    let mut out = Vec::new();
                    cond.push_into_children(&mut out);
                    out.push(pass);
                    out.push(fail);
                    out.into_boxed_slice()
                }
            },
        }
    }

    pub fn map<T>(self, f: &mut impl FnMut(N) -> T) -> Node<T> {
        match self {
            Node::Leaf(l) => Node::Leaf(l),

            Node::Branch(branch) => Node::Branch(match branch {
                Branch::Add(ns) => Branch::Add(ns.map(f)),
                Branch::Mul(ns) => Branch::Mul(ns.map(f)),
                Branch::Min(ns) => Branch::Min(ns.map(f)),
                Branch::Max(ns) => Branch::Max(ns.map(f)),

                Branch::Sin(n) => Branch::Sin(f(n)),
                Branch::Cos(n) => Branch::Cos(f(n)),
                Branch::Tan(n) => Branch::Tan(f(n)),

                Branch::Asin(n) => Branch::Asin(f(n)),
                Branch::Acos(n) => Branch::Acos(f(n)),
                Branch::Atan(n) => Branch::Atan(f(n)),

                Branch::Sinh(n) => Branch::Sinh(f(n)),
                Branch::Cosh(n) => Branch::Cosh(f(n)),
                Branch::Tanh(n) => Branch::Tanh(f(n)),

                Branch::Asinh(n) => Branch::Asinh(f(n)),
                Branch::Acosh(n) => Branch::Acosh(f(n)),
                Branch::Atanh(n) => Branch::Atanh(f(n)),

                Branch::Arg(n) => Branch::Arg(f(n)),
                Branch::Conj(n) => Branch::Conj(f(n)),
                Branch::Norm(n) => Branch::Norm(f(n)),
                Branch::Sign(n) => Branch::Sign(f(n)),

                Branch::Real(n) => Branch::Real(f(n)),
                Branch::Imag(n) => Branch::Imag(f(n)),

                Branch::Pow { base, exp } => {
                    Branch::Pow { base: f(base), exp: f(exp) }
                }

                Branch::Log { base, arg } => {
                    Branch::Log { base: f(base), arg: f(arg) }
                }

                Branch::Atan2 { a, b } => Branch::Atan2 { a: f(a), b: f(b) },

                Branch::Matrix(matrix) => Branch::Matrix(Matrix {
                    shape: matrix.shape,
                    elements: matrix.elements.into_iter().map(f).collect(),
                }),

                Branch::Transpose(n) => Branch::Transpose(f(n)),
                Branch::Det(n) => Branch::Det(f(n)),
                Branch::Rank(n) => Branch::Rank(f(n)),
                Branch::Trace(n) => Branch::Trace(f(n)),

                Branch::Conditional { cond, pass, fail } => {
                    Branch::Conditional {
                        cond: cond.map(&mut *f),
                        pass: f(pass),
                        fail: f(fail),
                    }
                }
            }),
        }
    }
}

impl<N> Branch<N> {
    pub fn as_binary(&self) -> Option<[&N; 2]> {
        match self {
            Branch::Add([a, b])
            | Branch::Mul([a, b])
            | Branch::Min([a, b])
            | Branch::Max([a, b]) => Some([a, b]),
            _ => None,
        }
    }

    pub fn as_binary_mut(&mut self) -> Option<[&mut N; 2]> {
        match self {
            Branch::Add([a, b])
            | Branch::Mul([a, b])
            | Branch::Min([a, b])
            | Branch::Max([a, b]) => Some([a, b]),
            _ => None,
        }
    }
}

impl<N> Condition<N> {
    pub fn map<T>(self, f: &mut impl FnMut(N) -> T) -> Condition<T> {
        match self {
            Condition::Eq(a, b) => Condition::Eq(f(a), f(b)),
            Condition::Ne(a, b) => Condition::Ne(f(a), f(b)),
            Condition::Lt(a, b) => Condition::Lt(f(a), f(b)),
            Condition::Le(a, b) => Condition::Le(f(a), f(b)),
            Condition::Gt(a, b) => Condition::Gt(f(a), f(b)),
            Condition::Ge(a, b) => Condition::Ge(f(a), f(b)),

            Condition::And(conditions) => Condition::And(
                conditions.into_iter().map(|c| c.map(f)).collect(),
            ),

            Condition::Or(conditions) => Condition::Or(
                conditions.into_iter().map(|c| c.map(f)).collect(),
            ),

            Condition::Not(condition) => {
                Condition::Not(Box::new(condition.map(f)))
            }
        }
    }

    pub fn children(&self) -> Box<[&N]> {
        let mut out = Vec::new();
        self.push_children(&mut out);
        out.into_boxed_slice()
    }

    pub fn children_mut(&mut self) -> Box<[&mut N]> {
        let mut out = Vec::new();
        self.push_children_mut(&mut out);
        out.into_boxed_slice()
    }

    pub fn into_children(self) -> Box<[N]> {
        let mut out = Vec::new();
        self.push_into_children(&mut out);
        out.into_boxed_slice()
    }

    // The `push_*` helpers write into a single shared buffer so that nested
    // And/Or/Not conditions don't allocate an intermediate slice per level.

    fn push_children<'a>(&'a self, out: &mut Vec<&'a N>) {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => {
                out.push(a);
                out.push(b);
            }

            Condition::And(conditions) | Condition::Or(conditions) => {
                for c in conditions.iter() {
                    c.push_children(out);
                }
            }

            Condition::Not(condition) => condition.push_children(out),
        }
    }

    fn push_children_mut<'a>(&'a mut self, out: &mut Vec<&'a mut N>) {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => {
                out.push(a);
                out.push(b);
            }

            Condition::And(conditions) | Condition::Or(conditions) => {
                for c in conditions.iter_mut() {
                    c.push_children_mut(out);
                }
            }

            Condition::Not(condition) => condition.push_children_mut(out),
        }
    }

    fn push_into_children(self, out: &mut Vec<N>) {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => {
                out.push(a);
                out.push(b);
            }

            Condition::And(conditions) | Condition::Or(conditions) => {
                for c in conditions.into_vec() {
                    c.push_into_children(out);
                }
            }

            Condition::Not(condition) => (*condition).push_into_children(out),
        }
    }

    pub fn hash_structure<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Condition::And(conds) | Condition::Or(conds) => {
                conds.len().hash(state);
                for c in conds.iter() {
                    c.hash_structure(state);
                }
            }
            Condition::Not(cond) => {
                cond.hash_structure(state);
            }
            _ => (),
        }
    }
}

impl<N> Matrix<N> {
    // pub fn from_fn(
    //     rows: impl Into<usize>,
    //     cols: impl Into<usize>,
    //     f: FnMut(usize, usize) -> Expr,
    // ) -> Matrix {
    // }

    pub fn from_elements(shape: Shape, elements: Box<[N]>) -> Matrix<N> {
        assert_eq!(shape.cols.get() * shape.rows.get(), elements.len());
        Self { shape, elements }
    }

    pub fn fill(rows: impl Into<usize>, cols: impl Into<usize>, el: N) -> Self
    where
        N: Clone, {
        let rows = rows.into();
        let cols = cols.into();
        Self {
            shape: Shape::rect(rows, cols),
            elements: vec![el; rows * cols].into_boxed_slice(),
        }
    }

    /// Returns (rows, cols) for this matrix
    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn rows(&self) -> NonZero<usize> {
        self.shape.rows
    }

    pub fn cols(&self) -> NonZero<usize> {
        self.shape.cols
    }

    pub fn elements(&self) -> &[N] {
        &self.elements
    }

    pub fn elements_mut(&mut self) -> &mut [N] {
        &mut self.elements
    }

    pub fn into_elements(self) -> Box<[N]> {
        self.elements
    }

    pub fn map<T>(self, f: &mut impl FnMut(N) -> T) -> Matrix<T> {
        Matrix {
            shape: self.shape,
            elements: self.elements.into_iter().map(f).collect(),
        }
    }

    pub fn into_map(self, f: &mut impl FnMut(N) -> N) -> Matrix<N> {
        Matrix {
            shape: self.shape,
            elements: self.elements.into_iter().map(f).collect(),
        }
    }
}

impl Leaf {
    pub fn shape(&self) -> Shape {
        match self {
            Leaf::Symbol(symbol) => symbol.shape(),
            Leaf::Constant(constant) => constant.quantity().value().shape(),
            Leaf::Quantity(quantity) => quantity.value().shape(),
        }
    }
}

impl<N> Index<usize> for Matrix<N> {
    type Output = [N];

    fn index(&self, row: usize) -> &Self::Output {
        let start = row * self.shape.cols.get();
        let end = start + self.shape.cols.get();
        &self.elements[start..end]
    }
}

impl<N> IndexMut<usize> for Matrix<N> {
    fn index_mut(&mut self, row: usize) -> &mut Self::Output {
        let start = row * self.shape.cols.get();
        let end = start + self.shape.cols.get();
        &mut self.elements[start..end]
    }
}

impl From<f64> for Leaf {
    fn from(value: f64) -> Self {
        Leaf::Quantity(value * Unitless)
    }
}

impl From<i64> for Leaf {
    fn from(value: i64) -> Self {
        Leaf::Quantity(value * Unitless)
    }
}

impl From<Complex64> for Leaf {
    fn from(value: Complex64) -> Self {
        Leaf::Quantity(value * Unitless)
    }
}

impl From<Value> for Leaf {
    fn from(value: Value) -> Self {
        Leaf::Quantity(value * Unitless)
    }
}

impl From<Quantity> for Leaf {
    fn from(value: Quantity) -> Self {
        Leaf::Quantity(value)
    }
}

impl From<Constant> for Leaf {
    fn from(s: Constant) -> Self {
        Leaf::Constant(s)
    }
}

impl From<Symbol> for Leaf {
    fn from(s: Symbol) -> Self {
        Leaf::Symbol(s)
    }
}

impl From<Variable> for Leaf {
    fn from(s: Variable) -> Self {
        Leaf::Symbol(s.symbol())
    }
}

impl From<Connector> for Leaf {
    fn from(s: Connector) -> Self {
        Leaf::Symbol(s.variable().symbol())
    }
}

impl From<VariableBuilder<'_>> for Leaf {
    fn from(s: VariableBuilder<'_>) -> Self {
        Leaf::Symbol(s.variable().symbol())
    }
}

impl From<ConnectorBuilder<'_>> for Leaf {
    fn from(s: ConnectorBuilder<'_>) -> Self {
        Leaf::Symbol(s.connector().variable().symbol())
    }
}

impl<T> From<&T> for Leaf
where
    Leaf: From<T>,
{
    fn from(s: &T) -> Self {
        s.clone().into()
    }
}

impl<N> From<Leaf> for Node<N> {
    fn from(l: Leaf) -> Self {
        Node::Leaf(l)
    }
}

impl<N, T> From<T> for Node<N>
where
    Leaf: From<T>,
{
    default fn from(l: T) -> Self {
        Node::Leaf(l.into())
    }
}
