use std::{
    hash::{DefaultHasher, Hash, Hasher, RandomState},
    ops::{Add, BitXor, Div, Mul, Neg, Sub},
    rc::Rc,
    sync::Arc,
};

use crate::{
    core::scalar::Scalar,
    dimension::Quantity,
    expr::{Binary, Expr, Node, Shaped, Variadic, ops::*},
    symbol::Symbol,
    system::var::Variable,
};

/* ---------------------------------- IMPLS --------------------------------- */

// impl From<Double> for Expr {
//     fn from(value: Double) -> Self {
//         Expr::Double(Rc::new(value))
//     }
// }

// impl From<Single> for Expr {
//     fn from(value: Single) -> Self {
//         Expr::Single(Rc::new(value))
//     }
// }
//
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
        match v {
            Variable::Unknown { symbol, guess } => Self::Symbol(v.symbol()),
            Variable::Known { symbol, value } => {
                Self::Const(value * symbol.unit())
            }
        }
    }
}

macro_rules! impl_op {
    ($t0:ty, $ty:ty, $op:ident, $method:ident, $expr:expr, normal) => {
        impl $op<$ty> for $t0 {
            type Output = Expr;

            fn $method(self, rhs: $ty) -> Expr {
                $expr(self.into(), rhs.into()).into()
            }
        }
    };
    ($t0:ty, $ty:ty, $op:ident, $method:ident, $expr:expr, symmetrical) => {
        impl $op<$ty> for $t0 {
            type Output = Expr;

            fn $method(self, rhs: $ty) -> Expr {
                $expr(self.into(), rhs.into()).into()
            }
        }

        impl $op<$t0> for $ty {
            type Output = Expr;

            fn $method(self, rhs: $t0) -> Expr {
                $expr(self.into(), rhs.into()).into()
            }
        }
    };
}

macro_rules! impl_expr_ops {
    (
        $t0:ty, [$($ty:ty),+ $(,)?], $config:tt
    ) => {
        $(
            impl_op!($t0, $ty, Add, add, |lhs: Expr, rhs: Expr| {
                assert_eq!(lhs.shape(), rhs.shape(), "Tried to add two expressions of different shapes: {lhs}, {rhs}");

                Variadic::Add(vec![lhs, rhs])
            }, $config);
            impl_op!($t0, $ty, Mul, mul, |lhs, rhs| Variadic::Mul(vec![lhs, rhs]), $config);
            impl_op!($t0, $ty, Div, div, |lhs: Expr, rhs: Expr| {
                assert!(
                    lhs.shape().cols == lhs.shape().rows || (lhs.shape() == rhs.shape() && lhs.shape().is_vec()),
                    "Matrix multiplication A * B requires A to have as many columns as B has rows.
                    A special case is when both A and B are vectors of equal shape, in which case Mul means dot product."
                );
                Variadic::Mul(vec![lhs, Binary::Pow(Pow { base: rhs, exp: (-1.0).into() }).into()])
            }, $config);
            impl_op!($t0, $ty, Sub, sub, |lhs, rhs: Expr| Variadic::Add(vec![lhs, -rhs]), $config);
            impl_op!($t0, $ty, BitXor, bitxor, |lhs: Expr, rhs: Expr| {
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

                Binary::Pow(Pow{
                    base: lhs,
                    exp: rhs
                })
            }, $config);

        )+
    };
}

impl_expr_ops!(&Expr, [i64, f64, Scalar, Quantity, Symbol], symmetrical);
impl_expr_ops!(&Expr, [&Expr], normal);

impl_expr_ops!(Expr, [i64, f64, Scalar, Quantity, Symbol, &Expr], symmetrical);
impl_expr_ops!(Expr, [Expr], normal);

impl_expr_ops!(Symbol, [i64, f64, Scalar, Quantity], symmetrical);
impl_expr_ops!(Symbol, [Symbol], normal);

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
