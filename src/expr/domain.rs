use std::ops::{Add, Mul};

use num::{Complex, bigint::Sign};
use ordered_float::Pow;

use super::{
    Node,
    tree::{Branch, Leaf},
};
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

    pub fn real(&self) -> Domain {}
    pub fn imag(&self) -> Domain {}

    pub fn min(&self, other: Domain) -> Domain {}
    pub fn max(&self, other: Domain) -> Domain {}
    pub fn union(&self, other: Domain) -> Domain {}
}

impl Pow<Domain> for Domain {
    type Output = Domain;

    fn pow(self, rhs: Domain) -> Self::Output {
        todo!()
    }
}

impl Add<Domain> for Domain {
    type Output = Domain;

    fn add(self, rhs: Domain) -> Self::Output {}
}

impl Mul<Domain> for Domain {
    type Output = Domain;

    fn mul(self, rhs: Domain) -> Self::Output {}
}

impl Expr {
    /// Returns the domain of this expression.
    /// The possible values of this expression are guaranteed to be contained in its domain, but its not required to cover all of it.
    pub fn domain(&self) -> Domain {
        self.fold_dfs(|_, node: &Node<Domain>| match node {
            Node::Leaf(leaf) => match leaf {
                Leaf::Symbol(symbol) => symbol.domain(),
                Leaf::Constant(constant) => {
                    constant.quantity().value().domain()
                }
                Leaf::Quantity(quantity) => quantity.value().domain(),
            },
            Node::Branch(branch) => match branch {
                Branch::Add([a, b]) => *a + *b,
                Branch::Mul([a, b]) => *a * *b,
                Branch::Min([a, b]) => a.min(*b),
                Branch::Max([a, b]) => a.max(*b),
                Branch::Sin(_) => todo!(),
                Branch::Cos(_) => todo!(),
                Branch::Tan(_) => todo!(),
                Branch::Asin(_) => todo!(),
                Branch::Acos(_) => todo!(),
                Branch::Atan(_) => todo!(),
                Branch::Sinh(_) => todo!(),
                Branch::Cosh(_) => todo!(),
                Branch::Tanh(_) => todo!(),
                Branch::Asinh(_) => todo!(),
                Branch::Acosh(_) => todo!(),
                Branch::Atanh(_) => todo!(),
                Branch::Arg(_) => Domain::REAL,
                Branch::Conj(_) => todo!(),
                Branch::Norm(_) => Domain::new(Interval::R_NN, Interval::ZERO),
                Branch::Sign(_) => todo!(),
                Branch::Real(x) => x.real(),
                Branch::Imag(x) => x.imag(),
                Branch::Pow { base, exp } => base.pow(*exp),
                Branch::Log { base, arg } => todo!(),
                Branch::Atan2 { a, b } => Domain::REAL,
                Branch::Matrix(matrix) => matrix
                    .elements()
                    .iter()
                    .copied()
                    .reduce(|a, b| a.union(b))
                    .unwrap(),
                Branch::Transpose(x) => *x,
                Branch::Det(_) => todo!(),
                Branch::Rank(_) => todo!(),
                Branch::Trace(_) => todo!(),
                Branch::Conditional { cond, pass, fail } => pass.union(*fail),
            },
        })
    }

    pub fn realize(self) -> [Expr; 2] {
        todo!()
    }
}
