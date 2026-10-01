use std::{
    cell::{LazyCell, OnceCell},
    ops::Neg,
    rc::Rc,
};

use num::{complex::Complex64, pow::Pow};

use crate::{
    core::{util::impl_op_permutations, value::Value},
    expr::{
        Expr, ExprNode, NodeId,
        tree::{Branch, Leaf, Node},
    },
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::{Quantity, Unit},
};

/* -------------------------------------------------------------------------- */

// impl_op_permutations! {
//     types = [
//         i64, f64, Complex64, &Complex64, Quantity, &Quantity, Value, &Value, Constant, &Constant, Symbol, &Symbol,
//         Variable, &Variable, Connector, &Connector, VariableBuilder<'_>, &VariableBuilder<'_>,
//         ConnectorBuilder<'_>, &ConnectorBuilder<'_>, Leaf, &Leaf
//     ],
//     exclude_permutations = [i64, f64, Quantity, &Quantity, Value, &Value, Complex64, &Complex64],
//     exclude_specific = [(Leaf, Leaf), (Leaf, &Leaf), (&Leaf, Leaf), (&Leaf, &Leaf)],
//     into = Leaf,
//     out = Expr,

//     add = {
//         assert_eq!(
//             lhs.shape(),
//             rhs.shape(),
//             "Tried to add two expressions of different shapes"
//         );

//         lhs + rhs
//     },

//     mul = {
//         assert!(
//             lhs.shape().cols == lhs.shape().rows
//                 || (lhs.shape() == rhs.shape() && lhs.shape().is_vec()),
//             "Matrix multiplication requires compatible shapes"
//         );

//         lhs * rhs
//     },

//     div = {
//         lhs * rhs.pow(Leaf::from(-1))
//     },

//     sub = {
//         lhs + (-rhs)
//     },

//     pow = {
//         assert!(
//             lhs.shape().is_square_mat() || lhs.shape().is_scalar(),
//             "Only square matrices can be raised to a power"
//         );

//         assert!(
//             rhs.shape().is_square_mat() || rhs.shape().is_scalar(),
//             "Only square matrices can be an exponent"
//         );

//         assert!(
//             !(lhs.shape().is_square_mat() && rhs.shape().is_square_mat()),
//             "Cannot raise a matrix to the power of another matrix yet"
//         );

//         lhs.pow(rhs)
//     },

//     partial_eq = {
//         lhs == rhs
//     }
// }

impl_op_permutations!(
    types = [
        i64, f64, Complex64, &Complex64, Quantity, &Quantity, Value, &Value, Constant, &Constant, Symbol, &Symbol,
        Variable, &Variable, Connector, &Connector, VariableBuilder<'_>, &VariableBuilder<'_>,
        ConnectorBuilder<'_>, &ConnectorBuilder<'_>, Leaf, &Leaf, Expr, &Expr
    ],
    exclude_permutations = [i64, f64, Quantity, &Quantity, Value, &Value, Complex64, &Complex64],
    exclude_specific = [(Expr, Expr)],
    into = Expr,
    out = Expr,

    add = {
        lhs + rhs
    },

    mul = {
        lhs * rhs
    },

    div = {
        lhs * rhs.pow(-1)
    },

    sub = {
        lhs + -rhs
    },

    pow = {
        lhs.pow(rhs)
    },

    partial_eq = {
        lhs == rhs
    }
);

/* -------------------------------------------------------------------------- */

impl Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Neg for &Expr {
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

impl Neg for &Symbol {
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

impl<T> From<T> for Expr
where
    T: Into<ExprNode>,
{
    fn from(value: T) -> Self {
        let mut expr = Self::new();
        expr.push_root(value.into());

        expr
    }
}
impl Expr {
    pub fn qty(&mut self, val: impl Into<Quantity>) -> NodeId {
        self.push(Node::Leaf(Leaf::Quantity(val.into())))
    }

    pub fn symbol(&mut self, symbol: Symbol) -> NodeId {
        self.push(Node::Leaf(Leaf::Symbol(symbol)))
    }

    pub fn constant(&mut self, constant: Constant) -> NodeId {
        self.push(Node::Leaf(Leaf::Constant(constant)))
    }

    pub fn zero(&mut self) -> NodeId {
        self.qty(0)
    }

    pub fn one(&mut self) -> NodeId {
        self.qty(1)
    }

    pub fn neg_one(&mut self) -> NodeId {
        self.qty(-1)
    }

    pub fn add(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Add([a, b])))
    }

    pub fn mul(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Mul([a, b])))
    }

    pub fn max(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Nax([a, b])))
    }

    pub fn min(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Min([a, b])))
    }

    pub fn neg(&mut self, a: NodeId) -> NodeId {
        let neg_1 = self.neg_one();
        self.mul(neg_1, a)
    }

    pub fn sub(&mut self, a: NodeId, b: NodeId) -> NodeId {
        let neg_b = self.neg(b);
        self.add(a, neg_b)
    }

    pub fn pow(&mut self, base: NodeId, exp: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Pow { base, exp }))
    }

    pub fn div(&mut self, num: NodeId, den: NodeId) -> NodeId {
        let neg_one = self.neg_one();
        let inv_den = self.pow(den, neg_one);
        self.mul(num, inv_den)
    }

    pub fn sqrt(&mut self, u: NodeId) -> NodeId {
        let half = self.qty(0.5);
        self.pow(u, half)
    }

    pub fn log(&mut self, base: NodeId, arg: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Log { base, arg }))
    }

    pub fn ln(&mut self, arg: NodeId) -> NodeId {
        let base_e = self.constant(e);
        self.log(base_e, arg)
    }

    pub fn atan2(&mut self, a: NodeId, b: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Atan2 { a, b }))
    }

    pub fn sin(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Sin(u)))
    }

    pub fn cos(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Cos(u)))
    }

    pub fn tan(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Tan(u)))
    }

    pub fn asin(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Asin(u)))
    }

    pub fn acos(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Acos(u)))
    }

    pub fn atan(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Atan(u)))
    }

    pub fn sinh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Sinh(u)))
    }

    pub fn cosh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Cosh(u)))
    }

    pub fn tanh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Tanh(u)))
    }

    pub fn asinh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Asinh(u)))
    }

    pub fn acosh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Acosh(u)))
    }

    pub fn atanh(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Atanh(u)))
    }

    pub fn transpose(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Transpose(u)))
    }

    pub fn conj(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Conj(u)))
    }

    pub fn real(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Real(u)))
    }

    pub fn imag(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Imag(u)))
    }

    pub fn sign(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Sign(u)))
    }

    pub fn trace(&mut self, u: NodeId) -> NodeId {
        self.push(Node::Branch(Branch::Trace(u)))
    }
}
