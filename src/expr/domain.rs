use num::{Complex, bigint::Sign};

use super::Node;
use crate::expr::{
    Expr,
    domain::Endpoint::{Neg, Pos, Zero},
};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Endpoint {
    Neg,
    Pos,
    Zero,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Edge {
    Closed(Endpoint),
    Open(Endpoint),
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Interval {
    from: Edge,
    to: Edge,
    over: Universe,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Universe {
    Integer,
    Real,
}

pub enum Numeric {
    Real,
    Imag,
    Complex,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Domain {
    pub re: Interval,
    pub im: Interval,
}

impl Interval {
    pub const ZERO: Self = Self {
        from: Edge::Closed(Zero),
        to: Edge::Closed(Zero),
        over: Universe::Real,
    };

    const R: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Open(Pos),
        over: Universe::Real,
    };

    pub const R_P: Self = Self {
        from: Edge::Open(Zero),
        to: Edge::Open(Pos),
        over: Universe::Real,
    };

    pub const R_NN: Self = Self {
        from: Edge::Closed(Zero),
        to: Edge::Open(Pos),
        over: Universe::Real,
    };

    pub const R_N: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Open(Zero),
        over: Universe::Real,
    };

    pub const R_NP: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Closed(Zero),
        over: Universe::Real,
    };

    const Z: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Open(Pos),
        over: Universe::Integer,
    };

    pub const Z_P: Self = Self {
        from: Edge::Open(Zero),
        to: Edge::Open(Pos),
        over: Universe::Integer,
    };

    pub const Z_NN: Self = Self {
        from: Edge::Closed(Zero),
        to: Edge::Open(Pos),
        over: Universe::Integer,
    };

    pub const Z_N: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Open(Zero),
        over: Universe::Integer,
    };

    pub const Z_NP: Self = Self {
        from: Edge::Open(Neg),
        to: Edge::Closed(Zero),
        over: Universe::Integer,
    };
}

impl Interval {
    pub fn has_zero(&self) -> bool {
        match (self.from, self.to) {
            (_, Edge::Open(Endpoint::Zero)) => false,
            (Edge::Open(Endpoint::Zero), _) => false,
            _ => true,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.from == Edge::Closed(Zero) && self.to == Edge::Closed(Zero)
    }

    pub fn has_pos(&self) -> bool {
        match (self.from, self.to) {
            (_, Edge::Open(Pos)) => true,
            _ => false,
        }
    }

    pub fn is_pos(&self) -> bool {
        self.from == Edge::Open(Zero) && self.to == Edge::Open(Pos)
    }

    pub fn has_neg(&self) -> bool {
        match (self.from, self.to) {
            (Edge::Open(Neg), _) => true,
            _ => false,
        }
    }

    pub fn is_neg(&self) -> bool {
        self.from == Edge::Open(Neg) && self.to == Edge::Open(Zero)
    }

    pub fn is_integer(&self) -> bool {
        if self.is_zero() { true } else { self.over == Universe::Integer }
    }
}

impl Domain {
    pub fn new(re: Interval, im: Interval) -> Self {
        Self { re, im }
    }

    pub fn numeric(&self) -> Numeric {
        if self.im == Interval::ZERO {
            Numeric::Real
        } else if self.re == Interval::ZERO {
            Numeric::Imag
        } else {
            Numeric::Complex
        }
    }

    pub fn has_zero(&self) -> bool {
        self.re.has_zero() && self.im.has_zero()
    }

    pub fn is_real(&self) -> bool {
        self.im.is_zero()
    }

    pub fn is_integer(&self) -> bool {
        self.im.is_zero() && self.re.is_integer()
    }

    pub fn is_imag(&self) -> bool {
        self.re.is_zero() && !self.im.is_zero()
    }

    pub const COMPLEX: Domain = Domain { re: Interval::R, im: Interval::R };

    pub const REAL: Domain = Domain { re: Interval::R, im: Interval::ZERO };

    pub const IMAG: Domain = Domain { re: Interval::ZERO, im: Interval::R };
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
            Node::Atan2 { a, b } => Domain::REAL,
            Node::Matrix(matrix) => todo!(),
            Node::Transpose(expr) => expr.domain(),
            Node::Det(expr) => todo!(),
            Node::Rank(expr) => todo!(),
            Node::Trace(expr) => todo!(),
            Node::Piecewise { arms, default } => todo!(),
        }
    }
    pub fn realize(self) -> [Expr; 2] {
        todo!()
    }
}
