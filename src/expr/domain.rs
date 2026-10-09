use std::ops::{Add, Mul, Sub};

use num::{Complex, bigint::Sign};
use ordered_float::Pow;

use super::{
    Node,
    tree::{Branch, Leaf},
};
use crate::{
    expr::{
        Expr, NodeId,
        domain::Endpoint::{Neg, Pos, Zero},
    },
    symbol::constants::e,
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

pub struct Realized {
    pub re: Expr,
    pub im: Expr,
}

impl Realized {
    pub fn unpack(self) -> [Expr; 2] {
        [self.re, self.im]
    }

    pub fn from_real(re: Expr) -> Self {
        Self { re, im: Expr::from(0) }
    }
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

    pub fn real(&self) -> Domain {
        Domain { re: self.re, im: Interval::ZERO }
    }
    pub fn imag(&self) -> Domain {
        Domain { re: Interval::ZERO, im: self.im }
    }
}

impl Interval {
    pub fn min(&self, other: Interval) -> Interval {
        todo!()
    }
    pub fn max(&self, other: Interval) -> Interval {
        todo!()
    }

    pub fn union(&self, other: Interval) -> Interval {
        todo!()
    }

    // Why do these take self? well uhh
    // well maybe we want actual intervals one day and then we will be able to have more precise bounds
    // without changing anything else
    #[inline(always)]
    pub fn sin(&self) -> Self {
        Self::R
    }

    pub fn cos(&self) -> Self {
        Self::R
    }

    #[inline(always)]
    pub fn sinh(&self) -> Self {
        // Sinh is positive when arg is positive, negative when arg is negative.
        self.clone()
    }

    #[inline(always)]
    pub fn cosh(&self) -> Self {
        Self::R_P
    }
}

impl Pow<Interval> for Interval {
    type Output = Interval;

    fn pow(self, rhs: Interval) -> Self::Output {
        todo!()
    }
}

impl Add<Interval> for Interval {
    type Output = Interval;

    fn add(self, rhs: Interval) -> Self::Output {
        todo!()
    }
}

impl Sub<Interval> for Interval {
    type Output = Interval;

    fn sub(self, rhs: Interval) -> Self::Output {
        todo!()
    }
}

impl std::ops::Neg for Interval {
    type Output = Interval;

    fn neg(self) -> Self::Output {
        todo!()
    }
}

impl Mul<Interval> for Interval {
    type Output = Interval;

    fn mul(self, rhs: Interval) -> Self::Output {
        let universe = if self.is_integer() && rhs.is_integer() {
            Universe::Integer
        } else {
            Universe::Real
        };

        todo!()
    }
}

impl Node<Domain> {
    pub fn domain(&self) -> Domain {
        match self {
            Node::Leaf(leaf) => match leaf {
                Leaf::Symbol(symbol) => symbol.domain(),
                Leaf::Constant(constant) => {
                    constant.quantity().value().domain()
                }
                Leaf::Quantity(quantity) => quantity.value().domain(),
            },
            Node::Branch(branch) => match branch {
                Branch::Add([a, b]) => Domain::new(a.re + b.re, a.im + b.im),
                Branch::Mul([a, b]) => Domain::new(
                    a.re * b.re - a.im * b.im,
                    a.re * b.im + a.im * b.re,
                ),
                Branch::Min([a, b]) => {
                    Domain::new(a.re.min(b.re), a.im.min(b.im))
                }
                Branch::Max([a, b]) => {
                    Domain::new(a.re.max(b.re), a.im.max(b.im))
                }
                Branch::Sin(u) => Domain::new(
                    u.re.sin() * u.im.cosh(),
                    u.re.cos() * u.im.sinh(),
                ),
                Branch::Cos(u) => Domain::new(
                    u.re.cos() * u.im.cosh(),
                    -(u.re.sin() * u.im.sinh()),
                ),
                Branch::Tan(u) => {
                    let u2_re: Interval = u.re * 2;
                    let u2_im: Interval = u.im * 2;

                    Domain::new(
                        (u2_re).sin() / (u2_re.cos() + u2_im.cosh()),
                        (u2_im).sinh() / (u2_re.cos() + u2_im.cosh()),
                    )
                }
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
                Branch::Conj(x) => Domain::new(x.re, -x.im),
                Branch::Norm(_) => Domain::new(Interval::R_NN, Interval::ZERO),
                Branch::Sign(_) => todo!(),
                Branch::Real(x) => x.real(),
                Branch::Imag(x) => Domain::new(x.im, Interval::ZERO),
                Branch::Pow { base, exp } => todo!(),
                Branch::Log { base, arg } => todo!(),
                Branch::Atan2 { x: a, y: b } => Domain::REAL,
                Branch::Matrix(matrix) => matrix
                    .elements()
                    .iter()
                    .copied()
                    .reduce(|a, b| {
                        Domain::new(a.re.union(b.re), a.im.union(b.im))
                    })
                    .unwrap(),
                Branch::Transpose(x) => *x,
                Branch::Det(_) => todo!(),
                Branch::Rank(_) => todo!(),
                Branch::Trace(_) => todo!(),
                Branch::Conditional { pass, fail, .. } => {
                    Domain::new(pass.re.union(fail.re), pass.im.union(fail.im))
                }
            },
        }
    }
}

impl Expr {
    /// Returns the domain of this expression.
    /// The possible values of this expression are guaranteed to be contained in its domain, but are not required to cover all of it.
    pub fn domain_of(&self, node: NodeId) -> Domain {
        self.fold_from(node, |_, _, node: Node<Domain>| node.domain())
    }

    pub fn domain(&self) -> Domain {
        self.domain_of(self.root)
    }

    pub fn realize(&self) -> Realized {
        let mut working = Expr::new();

        let (re_id, im_id) = self.fold(|_, old, node| {
            let ctx = working.edit();

            match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => (
                        symbol
                            .real()
                            .map_or_else(|| ctx.zero(), |s| ctx.symbol(s)),
                        symbol
                            .imag()
                            .map_or_else(|| ctx.zero(), |s| ctx.symbol(s)),
                    ),
                    Leaf::Constant(constant) => {
                        let [re_qty, im_qty] = constant.quantity().realize();

                        (ctx.qty(re_qty), ctx.qty(im_qty))
                    }
                    Leaf::Quantity(quantity) => {
                        let [re_qty, im_qty] = quantity.realize();

                        (ctx.qty(re_qty), ctx.qty(im_qty))
                    }
                },
                Node::Branch(branch) => match branch {
                    Branch::Add([(a_re, a_im), (b_re, b_im)]) => {
                        (ctx.add(a_re, b_re), ctx.add(a_im, b_im))
                    }
                    Branch::Mul([(a_re, a_im), (b_re, b_im)]) => (
                        ctx.sub(ctx.mul(a_re, b_re), ctx.mul(a_im, b_im)),
                        ctx.add(ctx.mul(a_re, b_im), ctx.mul(a_im, b_re)),
                    ),
                    Branch::Min([(a_re, a_im), (b_re, b_im)]) => {
                        (ctx.min(a_re, b_re), ctx.min(a_im, b_im))
                    }
                    Branch::Max([(a_re, a_im), (b_re, b_im)]) => {
                        (ctx.max(a_re, b_re), ctx.max(a_im, b_im))
                    }
                    Branch::Sin((u_re, u_im)) => (
                        ctx.mul(ctx.sin(u_re), ctx.cosh(u_im)),
                        ctx.mul(ctx.cos(u_re), ctx.sinh(u_im)),
                    ),
                    Branch::Cos((u_re, u_im)) => (
                        ctx.mul(ctx.cos(u_re), ctx.cosh(u_im)),
                        ctx.neg(ctx.mul(ctx.sin(u_re), ctx.sinh(u_im))),
                    ),
                    Branch::Tan((u_re, u_im)) => {
                        let u2_re = ctx.mul(ctx.qty(2), u_re);
                        let u2_im = ctx.mul(ctx.qty(2), u_im);

                        (
                            ctx.div(
                                ctx.sin(u2_re),
                                ctx.add(ctx.cos(u2_re), ctx.cosh(u2_im)),
                            ),
                            ctx.div(
                                ctx.sinh(u2_im),
                                ctx.add(ctx.cos(u2_re), ctx.cosh(u2_im)),
                            ),
                        )
                    }
                    Branch::Asin(_) => todo!(),
                    Branch::Acos(_) => todo!(),
                    Branch::Atan(_) => todo!(),
                    Branch::Sinh(_) => todo!(),
                    Branch::Cosh(_) => todo!(),
                    Branch::Tanh(_) => todo!(),
                    Branch::Asinh(_) => todo!(),
                    Branch::Acosh(_) => todo!(),
                    Branch::Atanh(_) => todo!(),
                    Branch::Arg(_) => todo!(),
                    Branch::Conj(_) => todo!(),
                    Branch::Norm(_) => todo!(),
                    Branch::Sign(_) => todo!(),
                    Branch::Real(_) => todo!(),
                    Branch::Imag(_) => todo!(),
                    Branch::Pow {
                        base: (b_re, b_im),
                        exp: (exp_re, exp_im),
                    } => {
                        let two = ctx.qty(2);
                        let euler = ctx.constant(e);

                        let ln_r = ctx.div(
                            ctx.ln(
                                ctx.add(ctx.pow(b_re, two), ctx.pow(b_im, two))
                            ),
                            two,
                        );

                        let tetha = ctx.atan2(b_re, b_im);

                        let a = ctx
                            .sub(ctx.mul(exp_re, ln_r), ctx.mul(exp_im, tetha));
                        let b = ctx
                            .add(ctx.mul(exp_im, ln_r), ctx.mul(exp_re, tetha));

                        (
                            ctx.mul(ctx.pow(euler, a), ctx.cos(b)),
                            ctx.mul(ctx.pow(euler, a), ctx.sin(b)),
                        )
                    }
                    Branch::Log {
                        base: (base_re, base_im),
                        arg: (arg_re, arg_im),
                    } => {
                        let two = ctx.qty(2);

                        let ln_z = |re: NodeId, im: NodeId| {
                            let r2 =
                                ctx.add(ctx.pow(re, two), ctx.pow(im, two));
                            (ctx.div(ctx.ln(r2), two), ctx.atan2(re, im))
                        };

                        let (p, q) = ln_z(arg_re, arg_im);

                        if try { *old.as_leaf()?.as_constant()? == e }
                            .unwrap_or(false)
                        {
                            (p, q)
                        } else {
                            let (r, s) = ln_z(base_re, base_im);
                            let den = ctx.add(ctx.pow(r, two), ctx.pow(s, two));

                            (
                                ctx.div(
                                    ctx.add(ctx.mul(p, r), ctx.mul(q, s)),
                                    den,
                                ),
                                ctx.div(
                                    ctx.sub(ctx.mul(q, r), ctx.mul(p, s)),
                                    den,
                                ),
                            )
                        }
                    }
                    Branch::Atan2 { x, y } => todo!(),
                    Branch::Matrix(matrix) => todo!(),
                    Branch::Transpose(_) => todo!(),
                    Branch::Det(_) => todo!(),
                    Branch::Rank(_) => todo!(),
                    Branch::Trace(_) => todo!(),
                    Branch::Conditional { cond, pass, fail } => todo!(),
                },
            }
        });

        let (mut re, mut im) = (Expr::new(), Expr::new());

        let re_root = re.import(&working, re_id);
        re.set_root(re_root);

        let im_root = im.import(&working, im_id);
        im.set_root(im_root);

        Realized { re: re.simplified(), im: im.simplified() }
    }
}
