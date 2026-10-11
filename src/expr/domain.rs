use std::ops::{Add, Div, Mul, Sub};

use float_eq::FloatEq;
use num::{Complex, bigint::Sign};
use ordered_float::Pow;
use smallvec::SmallVec;

use super::{
    Node,
    dag::{Branch, Leaf},
};
use crate::{
    core::value::EQ_ABS_TOL,
    expr::{
        Expr, NodeId,
        dag::{Conditional, Matrix},
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

pub struct Set(SmallVec<[Interval; 3]>);

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Domain {
    pub re: Interval,
    pub im: Interval,
}

impl Interval {
    pub const ZERO: Self = Self {
        from: Edge::Closed(Zero),
        to: Edge::Closed(Zero),
        over: Universe::Integer,
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

    pub fn ln(&self) -> Self {
        Self::R
    }

    pub fn atan2(&self, y: &Self) -> Self {
        Self::R
    }

    pub fn sign(&self) -> Self {
        *self
    }

    pub fn singleton(val: f64) -> Self {
        let universe = if val.round().eq_abs(&val, &EQ_ABS_TOL) {
            Universe::Integer
        } else {
            Universe::Real
        };

        if val.eq_abs(&0.0, &EQ_ABS_TOL) {
            Interval::ZERO
        } else if val > EQ_ABS_TOL {
            Interval {
                from: Edge::Open(Endpoint::Zero),
                to: Edge::Open(Endpoint::Pos),
                over: universe,
            }
        } else {
            Interval {
                from: Edge::Open(Endpoint::Neg),
                to: Edge::Open(Endpoint::Zero),
                over: universe,
            }
        }
    }
}

impl Add<Interval> for Interval {
    type Output = Interval;

    fn add(self, rhs: Interval) -> Self::Output {
        let universe = if self.is_integer() && rhs.is_integer() {
            Universe::Integer
        } else {
            Universe::Real
        };

        fn add_lower(a: Edge, b: Edge) -> Edge {
            if a == Edge::Open(Endpoint::Neg) || b == Edge::Open(Endpoint::Neg)
            {
                Edge::Open(Endpoint::Neg)
            } else if a == Edge::Open(Endpoint::Zero)
                || b == Edge::Open(Endpoint::Zero)
            {
                Edge::Open(Endpoint::Zero)
            } else {
                Edge::Closed(Endpoint::Zero)
            }
        }

        fn add_upper(a: Edge, b: Edge) -> Edge {
            if a == Edge::Open(Endpoint::Pos) || b == Edge::Open(Endpoint::Pos)
            {
                Edge::Open(Endpoint::Pos)
            } else if a == Edge::Open(Endpoint::Zero)
                || b == Edge::Open(Endpoint::Zero)
            {
                Edge::Open(Endpoint::Zero)
            } else {
                Edge::Closed(Endpoint::Zero)
            }
        }

        Interval {
            from: add_lower(self.from, rhs.from),
            to: add_upper(self.to, rhs.to),
            over: universe,
        }
    }
}

impl std::ops::Neg for Interval {
    type Output = Interval;

    fn neg(self) -> Self::Output {
        fn neg_endpoint(edge: Endpoint) -> Endpoint {
            match edge {
                Endpoint::Pos => Endpoint::Neg,
                Endpoint::Neg => Endpoint::Pos,
                Endpoint::Zero => Endpoint::Zero,
            }
        }

        fn neg_edge(edge: Edge) -> Edge {
            match edge {
                Edge::Closed(ep) => Edge::Closed(neg_endpoint(ep)),
                Edge::Open(ep) => Edge::Open(neg_endpoint(ep)),
            }
        }

        Interval {
            from: neg_edge(self.to),
            to: neg_edge(self.from),
            over: self.over,
        }
    }
}

impl Sub<Interval> for Interval {
    type Output = Interval;

    fn sub(self, rhs: Interval) -> Self::Output {
        self + (-rhs)
    }
}

impl Pow<Interval> for Interval {
    type Output = Interval;

    fn pow(self, rhs: Interval) -> Self::Output {
        // x^0 = 1
        if rhs.is_zero() {
            return Interval::singleton(1.0);
        }

        // 0^y = 0 for y > 0
        if self.is_zero() {
            return if rhs.is_pos() { Interval::ZERO } else { Interval::R };
        }

        let is_integer_domain = self.is_integer() && rhs.is_integer();

        if self.is_pos() {
            // Strictly positive base always yields positive results
            if is_integer_domain && !rhs.has_neg() {
                Interval::Z_P
            } else {
                Interval::R_P
            }
        } else if !self.has_neg() {
            // Non-negative base [0, +inf)
            if is_integer_domain && !rhs.has_neg() {
                Interval::Z_NN
            } else {
                Interval::R_NN
            }
        } else {
            // Base contains negative numbers
            if is_integer_domain { Interval::Z } else { Interval::R }
        }
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

        if self.is_zero() || rhs.is_zero() {
            return Interval {
                from: Edge::Closed(Endpoint::Zero),
                to: Edge::Closed(Endpoint::Zero),
                over: universe,
            };
        }

        fn edge_val(edge: Edge) -> (Endpoint, bool) {
            match edge {
                Edge::Closed(x) => (x, true),
                Edge::Open(x) => (x, false),
            }
        }

        fn mul_val(
            a: (Endpoint, bool),
            b: (Endpoint, bool),
        ) -> (Endpoint, bool) {
            if a.0 == Endpoint::Zero && a.1 {
                return (Endpoint::Zero, true);
            }
            if b.0 == Endpoint::Zero && b.1 {
                return (Endpoint::Zero, true);
            }

            let ep = match (a.0, b.0) {
                (Endpoint::Zero, _) | (_, Endpoint::Zero) => Endpoint::Zero,
                (Endpoint::Pos, Endpoint::Pos)
                | (Endpoint::Neg, Endpoint::Neg) => Endpoint::Pos,
                _ => Endpoint::Neg,
            };
            (ep, a.1 && b.1)
        }

        fn lower_rank(v: &(Endpoint, bool)) -> i32 {
            match v {
                (Endpoint::Neg, _) => 0,
                (Endpoint::Zero, true) => 1,
                (Endpoint::Zero, false) => 2,
                (Endpoint::Pos, _) => 3,
            }
        }

        fn upper_rank(v: &(Endpoint, bool)) -> i32 {
            match v {
                (Endpoint::Neg, _) => 0,
                (Endpoint::Zero, false) => 1,
                (Endpoint::Zero, true) => 2,
                (Endpoint::Pos, _) => 3,
            }
        }

        let p1 = mul_val(edge_val(self.from), edge_val(rhs.from));
        let p2 = mul_val(edge_val(self.from), edge_val(rhs.to));
        let p3 = mul_val(edge_val(self.to), edge_val(rhs.from));
        let p4 = mul_val(edge_val(self.to), edge_val(rhs.to));

        let products = [p1, p2, p3, p4];

        let min_val = *products.iter().min_by_key(|&x| lower_rank(x)).unwrap();
        let max_val = *products.iter().max_by_key(|&x| upper_rank(x)).unwrap();

        let from = if min_val.1 {
            Edge::Closed(min_val.0)
        } else {
            Edge::Open(min_val.0)
        };
        let to = if max_val.1 {
            Edge::Closed(max_val.0)
        } else {
            Edge::Open(max_val.0)
        };

        Interval { from, to, over: universe }
    }
}

impl Div<Interval> for Interval {
    type Output = Interval;

    fn div(self, rhs: Interval) -> Self::Output {
        // Approximate 1 / rhs by looking at the bounds.
        let inv_rhs = if rhs.is_pos()
            || (rhs.from == Edge::Closed(Endpoint::Zero)
                && rhs.to == Edge::Open(Endpoint::Pos))
        {
            Interval {
                from: Edge::Open(Endpoint::Zero),
                to: Edge::Open(Endpoint::Pos),
                over: Universe::Real,
            }
        } else if rhs.is_neg()
            || (rhs.from == Edge::Open(Endpoint::Neg)
                && rhs.to == Edge::Closed(Endpoint::Zero))
        {
            Interval {
                from: Edge::Open(Endpoint::Neg),
                to: Edge::Open(Endpoint::Zero),
                over: Universe::Real,
            }
        } else {
            // Covers crossing zero, or division by zero, yielding the widest possible domain.
            Interval {
                from: Edge::Open(Endpoint::Neg),
                to: Edge::Open(Endpoint::Pos),
                over: Universe::Real,
            }
        };

        let mut result = self * inv_rhs;
        result.over = Universe::Real;
        result
    }
}

impl From<i64> for Interval {
    fn from(value: i64) -> Self {
        Interval::singleton(value as f64)
    }
}

impl From<f64> for Interval {
    fn from(value: f64) -> Self {
        Interval::singleton(value)
    }
}

type DomainNode = Node<Domain, Domain, Matrix<Domain>, ()>;

impl DomainNode {
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
                    let u2_re: Interval = u.re * Interval::singleton(2.0);
                    let u2_im: Interval = u.im * Interval::singleton(2.0);

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
                Branch::Sign(x) => Domain::new(x.re.sign(), x.im.sign()),
                Branch::Real(x) => x.real(),
                Branch::Imag(x) => Domain::new(x.im, Interval::ZERO),
                Branch::Pow { base, exp } => {
                    let two = Interval::singleton(2.0);
                    let euler = Interval::singleton(std::f64::consts::E);
                    let ln_r = (base.re.pow(two) + base.im.pow(two)).ln() / two;
                    let θ = base.re.atan2(&base.im);
                    let a = exp.re * ln_r - exp.im * θ;
                    let b = exp.im * ln_r + exp.re * θ;

                    Domain::new(euler.pow(a) * b.cos(), euler.pow(a) * b.sin())
                }
                Branch::Log { base, arg } => {
                    let two = Interval::singleton(2.0);

                    let ln_z = |x: Domain| {
                        Domain::new(
                            (x.re.pow(two) + x.im.pow(two)).ln() / two,
                            x.re.atan2(&x.im),
                        )
                    };

                    let ln_arg = ln_z(*arg);
                    let ln_base = ln_z(*base);

                    let den = ln_base.re.pow(two) + ln_base.im.pow(two);

                    Domain::new(
                        (ln_arg.re * ln_base.re + ln_arg.im * ln_base.im) / den,
                        (ln_arg.im * ln_base.re - ln_arg.re * ln_base.im) / den,
                    )
                }
                Branch::Atan2 { x, y } => Domain::REAL,
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
                    Branch::Real((u_re, _)) => (u_re, ctx.zero()),
                    Branch::Imag((_, u_im)) => (ctx.zero(), u_im),
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

                        let θ = ctx.atan2(b_re, b_im);

                        let a =
                            ctx.sub(ctx.mul(exp_re, ln_r), ctx.mul(exp_im, θ));
                        let b =
                            ctx.add(ctx.mul(exp_im, ln_r), ctx.mul(exp_re, θ));

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
