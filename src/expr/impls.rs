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
    system::{Connector, Variable},
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
        Self::Symbol(v.symbol())
    }
}

impl From<&Variable> for Node {
    fn from(v: &Variable) -> Self {
        Self::Symbol(v.symbol())
    }
}

impl From<Connector> for Node {
    fn from(v: Connector) -> Self {
        Self::Symbol(v.variable().symbol())
    }
}

impl From<&Connector> for Node {
    fn from(v: &Connector) -> Self {
        Self::Symbol(v.variable().symbol())
    }
}

#[crabtime::function]
/// Impls A and B binary operations giving Expr, for every possible combination of the given types
fn impl_expr_ops(input: TokenStream) {
    #![dependency(proc-macro2 = "1")]
    #![dependency(syn = "2")]
    #![dependency(quote = "1")]
    #![dependency(itertools = "0.15")]

    use itertools::Itertools;
    use proc_macro2::*;
    use quote::ToTokens;
    use syn::{Token, parse::*, punctuated::Punctuated, *};
    let types =
        Punctuated::<Type, Token![,]>::parse_terminated.parse2(input).unwrap();

    for (a, b) in types.iter().cartesian_product(types.iter()) {
        let a = a.to_token_stream().to_string();
        let b = b.to_token_stream().to_string();

        if matches!(a.as_str(), "Scalar" | "f64" | "i64" | "Quantity")
            && matches!(b.as_str(), "Scalar" | "f64" | "i64" | "Quantity")
        {
            continue;
        }

        crabtime::output! {
            impl Add<{{b}}> for {{a}} {
                type Output = Expr;

                fn add(self, rhs: {{b}}) -> Expr {
                    let lhs = Expr::from(self);
                    let rhs = Expr::from(rhs);
                    assert_eq!(lhs.shape(), rhs.shape(), "Tried to add two expressions of different shapes: {lhs}, {rhs}");

                    Variadic::Add(vec![lhs, rhs]).into()
                }
            }

            impl Mul<{{b}}> for {{a}} {
                type Output = Expr;

                fn mul(self, rhs: {{b}}) -> Expr {
                    let lhs = Expr::from(self);
                    let rhs = Expr::from(rhs);
                    assert!(
                        lhs.shape().cols == lhs.shape().rows || (lhs.shape() == rhs.shape() && lhs.shape().is_vec()),
                        "Matrix multiplication A * B requires A to have as many columns as B has rows.
                        A special case is when both A and B are vectors of equal shape, in which case Mul means dot product."
                    );

                    Variadic::Mul(vec![lhs, rhs]).into()
                }
            }

            impl Div<{{b}}> for {{a}} {
                type Output = Expr;

                fn div(self, rhs: {{b}}) -> Expr {
                    self * Expr::from(Binary::Pow(Pow { base: rhs.into(), exp: (-1.0).into()}))
                }
            }

            impl Sub<{{b}}> for {{a}} {
                type Output = Expr;

                fn sub(self, rhs: {{b}}) -> Expr {
                    self + (-Expr::from(rhs))
                }
            }

            impl num::pow::Pow<{{b}}> for {{a}} {
                type Output = Expr;

                fn pow(self, rhs: {{b}}) -> Expr {
                    let lhs = Expr::from(self);
                    let rhs = Expr::from(rhs);

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
                    }).into()
                }
            }
        };
    }
}

impl_expr_ops!(
    i64, f64, Scalar, Quantity, Symbol, Expr, &Expr, Variable, &Variable,
    Connector, &Connector
);

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
