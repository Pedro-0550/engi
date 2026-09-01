use std::{
    hash::{DefaultHasher, Hash, Hasher, RandomState},
    ops::{
        Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign,
    },
    rc::Rc,
    sync::Arc,
};

use crate::{
    core::{
        util::impl_op_permutations,
        value::{Scalar, Value},
    },
    expr::{Binary, Expr, Node, Shaped, Variadic, ops::*},
    symbol::Symbol,
    system::{Connector, Variable},
    units::{Quantity, Unit},
};

/* ---------------------------------- IMPLS --------------------------------- */

impl<T> From<T> for Expr
where
    Node: From<T>,
{
    fn from(value: T) -> Self {
        let node: Node = value.into();
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        Self { node: Arc::new(node), hash: hasher.finish() }
    }
}

impl<T> From<&T> for Node
where
    Node: From<T>,
    T: Clone,
{
    fn from(value: &T) -> Self {
        value.clone().into()
    }
}

impl From<Scalar> for Expr {
    fn from(v: Scalar) -> Self {
        let node = Node::Const(v.into());
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        Self { node: Arc::new(node), hash: hasher.finish() }
    }
}

impl From<f64> for Expr {
    fn from(v: f64) -> Self {
        let node = Node::Const(v.into());
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        Self { node: Arc::new(node), hash: hasher.finish() }
    }
}

impl From<i64> for Expr {
    fn from(v: i64) -> Self {
        let node = Node::Const(v.into());
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        Self { node: Arc::new(node), hash: hasher.finish() }
    }
}

impl From<Variable> for Node {
    fn from(v: Variable) -> Self {
        Self::Symbol(v.symbol())
    }
}

impl From<Value> for Node {
    fn from(v: Value) -> Self {
        Self::Const(v * Unit::Unitless)
    }
}

impl From<Connector> for Node {
    fn from(v: Connector) -> Self {
        Self::Symbol(v.variable().symbol())
    }
}

impl_op_permutations! {
    types = [
        i64, f64, Scalar, Quantity, &Quantity, Value, &Value, Symbol, Expr,
        &Expr, Variable, &Variable, Connector, &Connector
    ],
    exclude_permutations = [i64, f64, Scalar, Quantity, &Quantity, Value, &Value],
    exclude_specific = [],
    out = Expr,

    add = {
        assert_eq!(
            lhs.shape(),
            rhs.shape(),
            "Tried to add two expressions of different shapes: {lhs}, {rhs}"
        );

        Variadic::Add(vec![lhs, rhs]).into()
    },

    mul = {
        assert!(
            lhs.shape().cols == lhs.shape().rows
                || (lhs.shape() == rhs.shape() && lhs.shape().is_vec()),
            "Matrix multiplication requires compatible shapes"
        );

        Variadic::Mul(vec![lhs, rhs]).into()
    },

    div = {
        lhs * pow(rhs, -1)
    },

    sub = {
        lhs + (-rhs)
    },

    pow = {
        assert!(
            lhs.shape().is_square() || lhs.shape().is_scalar(),
            "Only square matrices can be raised to a power"
        );

        assert!(
            rhs.shape().is_square() || rhs.shape().is_scalar(),
            "Only square matrices can be an exponent"
        );

        assert!(
            !(lhs.shape().is_square() && rhs.shape().is_square()),
            "Cannot raise a matrix to the power of another matrix yet"
        );

        Binary::Pow(Pow {
            base: lhs,
            exp: rhs,
        })
        .into()
    },

    partial_eq = {
        lhs == rhs
    }
}

impl Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Neg for Symbol {
    type Output = Expr;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl From<&Expr> for Expr {
    fn from(value: &Expr) -> Self {
        value.clone()
    }
}
