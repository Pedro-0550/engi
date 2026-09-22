use num::{Complex, bigint::Sign};

use super::Node;
use crate::expr::Expr;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Interval {
    Zero,
    /// An interval from 0 (inclusive) to positive infinity
    Positive,
    /// An interval from negative infinity to 0 (inclusive)
    Negative,
    /// An interval from negative infinity to positive infinity
    Universe,
}

pub enum Numeric {
    Real,
    Imag,
    Complex,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Domain {
    re: Interval,
    im: Interval,
}

impl Domain {
    pub fn new(re: Interval, im: Interval) -> Self {
        Self { re, im }
    }

    pub fn numeric(&self) -> Numeric {
        if self.im == Interval::Zero {
            Numeric::Real
        } else if self.re == Interval::Zero {
            Numeric::Imag
        } else {
            Numeric::Complex
        }
    }

    pub const COMPLEX: Domain =
        Domain { re: Interval::Universe, im: Interval::Universe };

    pub const REAL: Domain =
        Domain { re: Interval::Universe, im: Interval::Zero };

    pub const IMAG: Domain =
        Domain { re: Interval::Zero, im: Interval::Universe };
}

impl Expr {
    /// Returns the domain of this expression.
    /// The value of this expression is guaranteed to be contained in such domain, but its not required to cover all of it.
    pub fn domain(&self) -> Domain {
        match &self.node {
            Node::Symbol(symbol) => symbol.domain(),
            Node::Constant(constant) => constant.quantity().value().domain(),
            Node::Quantity(quantity) => quantity.value().domain(),

            Node::Add(exprs) => todo!(),
            Node::Mul(exprs) => todo!(),
            Node::Min(exprs) => todo!(),
            Node::Max(exprs) => todo!(),
            Node::Sin(expr) => todo!(),
            Node::Cos(expr) => todo!(),
            Node::Tan(expr) => todo!(),
            Node::Asin(expr) => todo!(),
            Node::Acos(expr) => todo!(),
            Node::Atan(expr) => todo!(),
            Node::Sinh(expr) => todo!(),
            Node::Cosh(expr) => todo!(),
            Node::Tanh(expr) => todo!(),
            Node::Asinh(expr) => todo!(),
            Node::Acosh(expr) => todo!(),
            Node::Atanh(expr) => todo!(),
            Node::Arg(expr) => todo!(),
            Node::Conj(expr) => todo!(),
            Node::Norm(expr) => todo!(),
            Node::Sign(expr) => todo!(),
            Node::Real(expr) => todo!(),
            Node::Imag(expr) => todo!(),
            Node::Pow { base, exp } => todo!(),
            Node::Log { base, arg } => todo!(),
            Node::Atan2 { a, b } => todo!(),
            Node::Matrix(matrix) => todo!(),
            Node::Transpose(expr) => todo!(),
            Node::Det(expr) => todo!(),
            Node::Rank(expr) => todo!(),
            Node::Trace(expr) => todo!(),
            Node::Piecewise { cond, pass, fail } => todo!(),
        }
    }
    pub fn realize(self) -> Complex<Expr> {
        todo!()
    }
}
