use std::{
    hash::Hash,
    ops::{Add, Div, Mul, Neg, Sub},
};

use num::{complex::Complex64, pow::Pow};

use crate::{
    core::{util::impl_op_permutations, value::Value},
    expr::{
        BuiltOperand, EditContext, Expr, Operand,
        dag::{Leaf, Operation},
    },
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::Quantity,
};

/* -------------------------------------------------------------------------- */

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

    // partial_eq = {
    //     lhs == rhs
    // }
);

/* -------------------------------------------------------------------------- */

impl<W: Hash + Eq + Copy> Mul<Expr<W>> for Expr<W> {
    type Output = Expr<W>;

    fn mul(mut self, rhs: Expr<W>) -> Self::Output {
        let a_id = self.root().unwrap();
        let b_id = self.import(&rhs, rhs.root().unwrap());
        self.push_root(Operand::Operation(Operation::Mul([a_id, b_id])));
        self
    }
}

impl<W: Hash + Eq + Copy> Add<Expr<W>> for Expr<W> {
    type Output = Expr<W>;

    fn add(mut self, rhs: Expr<W>) -> Self::Output {
        let a_id = self.root().unwrap();
        let b_id = self.import(&rhs, rhs.root().unwrap());
        self.push_root(Operand::Operation(Operation::Add([a_id, b_id])));
        self
    }
}

impl<W: Hash + Eq + Copy> Sub<Expr<W>> for Expr<W> {
    type Output = Expr<W>;

    fn sub(self, rhs: Expr<W>) -> Self::Output {
        self + -rhs
    }
}

impl<W: Hash + Eq + Copy> Div<Expr<W>> for Expr<W> {
    type Output = Expr<W>;

    fn div(self, rhs: Expr<W>) -> Self::Output {
        self * rhs.pow(Expr::<W>::from(-1))
    }
}

impl<W: Hash + Eq + Copy> Pow<Expr<W>> for Expr<W> {
    type Output = Expr<W>;

    fn pow(mut self, exp: Expr<W>) -> Self::Output {
        let base = self.root().unwrap();
        let exp = self.import(&exp, exp.root().unwrap());
        self.push_root(Operand::Operation(Operation::Pow { base, exp }));
        self
    }
}

impl<W: Hash + Eq + Copy> Neg for Expr<W> {
    type Output = Expr<W>;

    fn neg(self) -> Self::Output {
        Expr::<W>::from(-1) * self
    }
}

impl<W: Hash + Eq + Copy> Neg for &Expr<W> {
    type Output = Expr<W>;

    fn neg(self) -> Self::Output {
        Expr::<W>::from(-1) * self.clone()
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

impl<W: Copy + Eq + Hash> From<&Expr<W>> for Expr<W> {
    fn from(value: &Expr<W>) -> Self {
        value.clone()
    }
}

impl<W, T> From<T> for Expr<W>
where
    T: Into<BuiltOperand<W>>,
    W: Copy + Eq + Hash,
{
    fn from(value: T) -> Self {
        let mut expr = Self::new();
        expr.push_root(value.into());

        expr
    }
}

// impl From<Wildcard> for BuiltOperand<Wildcard> {
//     fn from(value: Wildcard) -> Self {
//         Operand::Wildcard(value)
//     }
// }

impl EditContext<'_> {
    fn push(&self, operand: BuiltOperand) -> Operand {
        let expr = self.expr.take().expect("Edit context used recursively");

        let result = expr.push(operand);

        self.expr.set(Some(expr));
        result
    }

    pub fn qty(&self, val: impl Into<Quantity>) -> Operand {
        self.push(Operand::Leaf(Leaf::Quantity(val.into())))
    }

    pub fn symbol(&self, symbol: Symbol) -> Operand {
        self.push(Operand::Leaf(Leaf::Symbol(symbol)))
    }

    pub fn constant(&self, constant: Constant) -> Operand {
        self.push(Operand::Leaf(Leaf::Constant(constant)))
    }

    pub fn import(&self, src: &EditContext<'_>, id: Operand) -> Operand {
        let expr = self.expr.take().expect("Context used recursively");
        let src_expr = src.expr.take().expect("Context used recursively");

        let result = expr.import(src_expr, id);

        src.expr.set(Some(src_expr));
        self.expr.set(Some(expr));
        result
    }

    pub fn zero(&self) -> Operand {
        self.qty(0)
    }

    pub fn one(&self) -> Operand {
        self.qty(1)
    }

    pub fn neg_one(&self) -> Operand {
        self.qty(-1)
    }

    pub fn add(&self, a: Operand, b: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Add([a, b])))
    }

    pub fn mul(&self, a: Operand, b: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Mul([a, b])))
    }

    pub fn max(&self, a: Operand, b: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Max([a, b])))
    }

    pub fn min(&self, a: Operand, b: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Min([a, b])))
    }

    pub fn neg(&self, a: Operand) -> Operand {
        let neg_1 = self.neg_one();
        self.mul(neg_1, a)
    }

    pub fn sub(&self, a: Operand, b: Operand) -> Operand {
        let neg_b = self.neg(b);
        self.add(a, neg_b)
    }

    pub fn pow(&self, base: Operand, exp: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Pow { base, exp }))
    }

    pub fn div(&self, num: Operand, den: Operand) -> Operand {
        let neg_one = self.neg_one();
        let inv_den = self.pow(den, neg_one);
        self.mul(num, inv_den)
    }

    pub fn sqrt(&self, u: Operand) -> Operand {
        let half = self.qty(0.5);
        self.pow(u, half)
    }

    pub fn log(&self, base: Operand, arg: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Log { base, arg }))
    }

    pub fn ln(&self, arg: Operand) -> Operand {
        let base_e = self.constant(e);
        self.log(base_e, arg)
    }

    pub fn atan2(&self, x: Operand, y: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Atan2 { x, y }))
    }

    pub fn sin(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Sin(u)))
    }

    pub fn cos(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Cos(u)))
    }

    pub fn tan(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Tan(u)))
    }

    pub fn asin(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Asin(u)))
    }

    pub fn acos(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Acos(u)))
    }

    pub fn atan(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Atan(u)))
    }

    pub fn sinh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Sinh(u)))
    }

    pub fn cosh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Cosh(u)))
    }

    pub fn tanh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Tanh(u)))
    }

    pub fn asinh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Asinh(u)))
    }

    pub fn acosh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Acosh(u)))
    }

    pub fn atanh(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Atanh(u)))
    }

    pub fn transpose(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Transpose(u)))
    }

    pub fn conj(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Conj(u)))
    }

    pub fn real(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Real(u)))
    }

    pub fn imag(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Imag(u)))
    }

    pub fn sign(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Sign(u)))
    }

    pub fn trace(&self, u: Operand) -> Operand {
        self.push(Operand::Operation(Operation::Trace(u)))
    }
}
