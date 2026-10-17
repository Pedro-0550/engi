use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZero,
    ops::{Index, IndexMut},
    slice, vec,
};

use ahash::AHashMap;
use derive_more::{Deref, From, Into, IsVariant};
use kinded::Kinded;
use num::complex::Complex64;

use crate::{
    core::{consed::Consed, index::Size, util::impl_as_variant, value::Value},
    expr::{Operand, dag, domain::Domain, shape::Shape},
    id,
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    simplify::{ClassId, pattern::Wildcard},
    symbol::{Symbol, constants::Constant},
    units::{Quantity, Unit::Unitless},
};

macro_rules! unary {
    ($n:ident) => {
        Operation::Sin($n)
            | Operation::Cos($n)
            | Operation::Tan($n)
            | Operation::Asin($n)
            | Operation::Acos($n)
            | Operation::Atan($n)
            | Operation::Sinh($n)
            | Operation::Cosh($n)
            | Operation::Tanh($n)
            | Operation::Asinh($n)
            | Operation::Acosh($n)
            | Operation::Atanh($n)
            | Operation::Arg($n)
            | Operation::Conj($n)
            | Operation::Norm($n)
            | Operation::Sign($n)
            | Operation::Real($n)
            | Operation::Imag($n)
            | Operation::Transpose($n)
            | Operation::Det($n)
            | Operation::Rank($n)
            | Operation::Trace($n)
    };
}

macro_rules! binary {
    ($a:ident, $b:ident) => {
        Operation::Pow { base: $a, exp: $b }
            | Operation::Log { base: $b, arg: $a }
            | Operation::Atan2 { x: $a, y: $b }
            | Operation::Add([$a, $b])
            | Operation::Mul([$a, $b])
            | Operation::Min([$a, $b])
            | Operation::Max([$a, $b])
    };
}

/* -------------------------------------------------------------------------- */

/// A kind of DAG node which is formed from some number of operands,
/// representing some operation or composition over them.
///
/// Contains every operation available in engi.
#[derive(Eq, PartialEq, Clone, Copy, Hash, Kinded, IsVariant)]
#[kinded(derive(Hash))]
pub enum Operation<O, C> {
    // Arithmetic
    Add([O; 2]),
    Mul([O; 2]),

    // Comparison
    Min([O; 2]),
    Max([O; 2]),

    // Trig
    Sin(O),
    Cos(O),
    Tan(O),

    Asin(O),
    Acos(O),
    Atan(O),
    Atan2 { x: O, y: O },

    // Hyperbolic trig
    Sinh(O),
    Cosh(O),
    Tanh(O),

    Asinh(O),
    Acosh(O),
    Atanh(O),

    // Complex
    Arg(O),
    Conj(O),
    Real(O),
    Imag(O),

    // Dunno what to call these
    Norm(O),
    Sign(O),

    // Exp
    Pow { base: O, exp: O },
    Log { base: O, arg: O },

    // Matrix
    Transpose(O),
    Det(O),
    Rank(O),
    Trace(O),

    // Piecewise
    Conditional { cond: C, pass: O, fail: O },
}

const _: () = assert!(size_of::<Operation<Operand, ConditionId>>() <= 10);

/// Dynamically sized row-major matrix
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Matrix<O> {
    shape: Shape,
    elements: Box<[O]>,
}

/// A boolean condition based on comparisons between the values of given nodes, or some logical composition over other conditions.
/// The type parameters should either be references/handles to the actual types (like [`ConditionId`] for [`Condition`]),
/// the type itself with some indirection (like [`Box<Condition>`]), or
/// or an intermediate type when used in folding operations.
///
/// "Flattening" a condition expression into an actual tree can be done with [`BoxedCondition`]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Condition<O, C> {
    // Comparison
    Eq(O, O),
    Ne(O, O),
    Lt(O, O),
    Le(O, O),
    Gt(O, O),
    Ge(O, O),

    // Composition
    And([C; 2]),
    Or([C; 2]),
    Not(C),
}

/// Leaf node in the expression DAG
/// Constants and quantities are not equal even if their underlying values are equal.
/// Constants are treated exactly like symbols up until evaluation, except that they cannot be used in substitutions.
#[derive(PartialEq, Clone, Eq, Hash, Kinded, IsVariant)]
#[kinded(derive(Hash))]
pub enum Leaf {
    Symbol(Symbol),
    Constant(Constant),
    Quantity(Quantity),
}

type FlattenedOperand = Operand<BoxedOperation, Leaf, BoxedMatrix>;
pub struct BoxedMatrix(Matrix<FlattenedOperand>);
pub struct BoxedOperation(Box<Operation<FlattenedOperand, BoxedCondition>>);
pub struct BoxedCondition(Box<Condition<FlattenedOperand, BoxedCondition>>);

/// Mathematical expression DAG.
/// Operations, conditions, matrices and leafs are hashconsed and referenced with strongly typed Ids.
///
/// When used with the DAG, [`Node`]s, [`Condition`]s, and every Id are lightweight and [`Copy`]
#[derive(Clone, Default)]
pub struct Dag<O: Hash + Eq, OKey, LKey, MKey> {
    pub(crate) operations: Consed<Operation<O, ConditionId>, OKey, OperationId>,
    pub(crate) leaves: Consed<Leaf, LKey, LeafId>,
    pub(crate) matrices: Consed<Matrix<O>, MKey, MatrixId>,
    pub(crate) conditions: Consed<Condition<O, ConditionId>, ConditionId>,

    pub(crate) root: Option<O>,
}

id!(
    /// An unique index for a [`Leaf`], valid only within the same [`Dag`]
    LeafId
);

id!(
    /// An unique index for an [`Operation`], valid only within the same [`Dag`]
    OperationId
);

id!(
    /// An unique index for a [`Matrix`], valid only within the same [`Dag`]
    MatrixId
);

id!(
    /// An unique index for a [`Condition`], valid only within the same [`Dag`]
    ConditionId
);

/* -------------------------------------------------------------------------- */

impl_as_variant!(Leaf, [Symbol => Symbol, Quantity => Quantity, Constant => Constant]);

impl<O, OKey, LKey, MKey> Dag<O, OKey, LKey, MKey>
where
    O: Hash + Eq,
{
    pub fn new() -> Self {
        Self {
            operations: Consed::default(),
            leaves: Consed::default(),
            matrices: Consed::default(),
            conditions: Consed::default(),
            root: None,
        }
    }
}

impl<O, C> Operation<O, C> {
    pub fn len(&self) -> u8 {
        match self {
            binary!(a, b) => 2,
            unary!(x) => 1,
            Operation::Conditional { pass, fail, .. } => 2,
        }
    }

    pub fn for_each_operand(&self, mut f: impl FnMut(&O)) {
        match self {
            binary!(a, b) => {
                f(a);
                f(b)
            }
            unary!(x) => f(x),
            Operation::Conditional { pass, fail, .. } => {
                f(pass);
                f(fail)
            }
        }
    }

    pub fn operand(&self, at: u8) -> Option<&O> {
        match self {
            binary!(a, b) => [a, b].get(at as usize).copied(),
            unary!(x) => {
                if at == 0 {
                    Some(x)
                } else {
                    None
                }
            }
            Operation::Conditional { pass, fail, .. } => {
                [pass, fail].get(at as usize).copied()
            }
        }
    }

    pub fn collect_operands<'s>(&'s self, into: &mut Vec<&'s O>) {
        match self {
            binary!(a, b) => {
                into.push(a);
                into.push(b);
            }
            unary!(x) => {
                into.push(x);
            }
            Operation::Conditional { pass, fail, .. } => {
                into.push(pass);
                into.push(fail);
            }
        }
    }

    pub fn condition(&self) -> Option<&C> {
        match self {
            Operation::Conditional { cond, .. } => Some(cond),
            _ => None,
        }
    }

    /// Maps this operations's children (operands and conditions) into some other types using the given functions.
    /// Returns the operation with all children mapped.
    pub fn map<NewO, NewC>(
        &self,
        mut o: impl FnMut(&O) -> NewO,
        mut c: impl FnMut(&C) -> NewC,
    ) -> Operation<NewO, NewC> {
        self.map_with(&mut (), |_, op| o(op), |_, cond| c(cond))
    }

    /// Like [`Operation::map`], except it can also take and pass a mutable state to the mapping functions.
    ///
    /// This is useful when the mapping depends on mutating some exterior state for both conditions and operands.
    /// The borrow checker forbids accessing the state mutably to both closures at once, so you need to take it as an argument
    pub fn map_with<S, NewO, NewC>(
        &self,
        s: &mut S,

        mut o: impl FnMut(&mut S, &O) -> NewO,
        mut c: impl FnMut(&mut S, &C) -> NewC,
    ) -> Operation<NewO, NewC> {
        match self {
            Operation::Add([a, b]) => Operation::Add([o(s, a), o(s, b)]),
            Operation::Mul([a, b]) => Operation::Mul([o(s, a), o(s, b)]),
            Operation::Min([a, b]) => Operation::Min([o(s, a), o(s, b)]),
            Operation::Max([a, b]) => Operation::Max([o(s, a), o(s, b)]),
            Operation::Sin(a) => Operation::Sin(o(s, a)),
            Operation::Cos(a) => Operation::Cos(o(s, a)),
            Operation::Tan(a) => Operation::Tan(o(s, a)),
            Operation::Asin(a) => Operation::Asin(o(s, a)),
            Operation::Acos(a) => Operation::Acos(o(s, a)),
            Operation::Atan(a) => Operation::Atan(o(s, a)),
            Operation::Atan2 { x, y } => {
                Operation::Atan2 { x: o(s, x), y: o(s, y) }
            }
            Operation::Sinh(a) => Operation::Sinh(o(s, a)),
            Operation::Cosh(a) => Operation::Cosh(o(s, a)),
            Operation::Tanh(a) => Operation::Tanh(o(s, a)),
            Operation::Asinh(a) => Operation::Asinh(o(s, a)),
            Operation::Acosh(a) => Operation::Acosh(o(s, a)),
            Operation::Atanh(a) => Operation::Atanh(o(s, a)),
            Operation::Arg(a) => Operation::Arg(o(s, a)),
            Operation::Conj(a) => Operation::Conj(o(s, a)),
            Operation::Real(a) => Operation::Real(o(s, a)),
            Operation::Imag(a) => Operation::Imag(o(s, a)),
            Operation::Norm(a) => Operation::Norm(o(s, a)),
            Operation::Sign(a) => Operation::Sign(o(s, a)),
            Operation::Pow { base, exp } => {
                Operation::Pow { base: o(s, base), exp: o(s, exp) }
            }
            Operation::Log { base, arg } => {
                Operation::Log { base: o(s, base), arg: o(s, arg) }
            }
            Operation::Transpose(a) => Operation::Transpose(o(s, a)),
            Operation::Det(a) => Operation::Det(o(s, a)),
            Operation::Rank(a) => Operation::Rank(o(s, a)),
            Operation::Trace(a) => Operation::Trace(o(s, a)),
            Operation::Conditional { cond, pass, fail } => {
                Operation::Conditional {
                    cond: c(s, cond),
                    pass: o(s, pass),
                    fail: o(s, fail),
                }
            }
        }
    }
}

impl<O, C> Condition<O, C> {
    pub fn operand(&self, at: usize) -> Option<&O> {
        match self {
            Condition::Eq(a, b)
            | Condition::Ne(a, b)
            | Condition::Lt(a, b)
            | Condition::Le(a, b)
            | Condition::Gt(a, b)
            | Condition::Ge(a, b) => [a, b].get(at).copied(),
            _ => None,
        }
    }

    pub fn condition(&self, at: usize) -> Option<&C> {
        match self {
            Condition::And(conds) | Condition::Or(conds) => conds.get(at),
            Condition::Not(x) if at == 0 => Some(x),
            _ => None,
        }
    }

    /// Maps this condition's children (operands and conditions) into some other types using the given functions.
    /// Returns the condition with all children mapped.
    pub fn map<NewO, NewC>(
        &self,
        mut o: impl FnMut(&O) -> NewO,
        mut c: impl FnMut(&C) -> NewC,
    ) -> Condition<NewO, NewC> {
        self.map_with(&mut (), |_, op| o(op), |_, cond| c(cond))
    }

    /// Like [`Condition::map`], except it can also take and pass a mutable state to the mapping functions.
    ///
    /// This is useful when the mapping depends on mutating some exterior state for both conditions and operands.
    /// The borrow checker forbids accessing the state mutably to both closures at once, so you need to take it as an argument
    pub fn map_with<S, NewO, NewC>(
        &self,
        s: &mut S,
        mut o: impl FnMut(&mut S, &O) -> NewO,
        mut c: impl FnMut(&mut S, &C) -> NewC,
    ) -> Condition<NewO, NewC> {
        match self {
            Condition::Eq(a, b) => Condition::Eq(o(s, a), o(s, b)),
            Condition::Ne(a, b) => Condition::Ne(o(s, a), o(s, b)),
            Condition::Lt(a, b) => Condition::Lt(o(s, a), o(s, b)),
            Condition::Le(a, b) => Condition::Le(o(s, a), o(s, b)),
            Condition::Gt(a, b) => Condition::Gt(o(s, a), o(s, b)),
            Condition::Ge(a, b) => Condition::Ge(o(s, a), o(s, b)),
            Condition::And([a, b]) => Condition::And([c(s, a), c(s, b)]),
            Condition::Or([a, b]) => Condition::Or([c(s, a), c(s, b)]),
            Condition::Not(x) => Condition::Not(c(s, x)),
        }
    }
}

impl<O> Matrix<O> {
    // pub fn from_fn(
    //     rows: impl Into<usize>,
    //     cols: impl Into<usize>,
    //     f: FnMut(usize, usize) -> Expr,
    // ) -> Matrix {
    // }

    pub fn from_elements(shape: Shape, elements: Box<[O]>) -> Matrix<O> {
        assert_eq!(shape.cols.get() * shape.rows.get(), elements.len() as Size);
        Self { shape, elements }
    }

    pub fn fill(rows: impl Into<Size>, cols: impl Into<Size>, with: O) -> Self
    where
        O: Clone, {
        let rows = rows.into();
        let cols = cols.into();
        Self {
            shape: Shape::rect(rows, cols),
            elements: vec![with; (rows * cols) as usize].into_boxed_slice(),
        }
    }

    /// Returns the shape of this matrix
    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn rows(&self) -> NonZero<Size> {
        self.shape.rows
    }

    pub fn cols(&self) -> NonZero<Size> {
        self.shape.cols
    }

    pub fn elements(&self) -> &[O] {
        &self.elements
    }

    pub fn elements_mut(&mut self) -> &mut [O] {
        &mut self.elements
    }

    pub fn into_elements(self) -> Box<[O]> {
        self.elements
    }

    pub fn map<T>(&self, f: impl FnMut(&O) -> T) -> Matrix<T> {
        Matrix {
            shape: self.shape,
            elements: self.elements.iter().map(f).collect(),
        }
    }

    pub fn into_map<T>(self, f: impl FnMut(O) -> T) -> Matrix<T> {
        Matrix {
            shape: self.shape,
            elements: self.elements.into_iter().map(f).collect(),
        }
    }
}

impl<O> Index<usize> for Matrix<O> {
    type Output = [O];

    fn index(&self, row: usize) -> &Self::Output {
        let start = (row as Size * self.shape.cols.get()) as usize;
        let end = (start as Size + self.shape.cols.get()) as usize;
        &self.elements[start..end]
    }
}

impl<O> IndexMut<usize> for Matrix<O> {
    fn index_mut(&mut self, row: usize) -> &mut Self::Output {
        let start = (row as Size * self.shape.cols.get()) as usize;
        let end = (start as Size + self.shape.cols.get()) as usize;
        &mut self.elements[start..end]
    }
}

impl Leaf {
    pub fn shape(&self) -> Shape {
        match self {
            Leaf::Constant(constant) => constant.quantity().value().shape(),
            Leaf::Quantity(quantity) => quantity.value().shape(),
            Leaf::Symbol(symbol) => symbol.shape(),
        }
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

impl<T: Clone> From<&T> for Leaf
where
    Leaf: From<T>,
{
    fn from(s: &T) -> Self {
        s.clone().into()
    }
}

impl<W, O, T, M> From<T> for Operand<W, O, Leaf, M>
where
    T: Into<Leaf>,
{
    fn from(l: T) -> Self {
        Operand::Leaf(l.into())
    }
}

impl<O, L, M> From<Wildcard> for Operand<Wildcard, O, L, M> {
    fn from(w: Wildcard) -> Self {
        Operand::Wildcard(w)
    }
}

// impl<N, T> From<T> for Node<N>
// where
//     Leaf: From<T>,
// {
//     default fn from(l: T) -> Self {
//         Node::Leaf(l.into())
//     }
// }
