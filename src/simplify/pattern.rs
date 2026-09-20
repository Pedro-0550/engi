/* --------------------------------- STRUCTS -------------------------------- */

use crate::{expr::Expr, units::Quantity};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Wildcard {
    name: &'static str,
}

#[derive(PartialEq, Clone, Eq)]
pub enum Pattern {
    Wildcard(Wildcard),
    Quantity(Quantity),

    Add(Vec<Pattern>),
    Mul(Vec<Pattern>),
    Min(Vec<Pattern>),
    Max(Vec<Pattern>),

    Sin(Box<Pattern>),
    Cos(Box<Pattern>),
    Tan(Box<Pattern>),

    Asin(Box<Pattern>),
    Acos(Box<Pattern>),
    Atan(Box<Pattern>),

    Sinh(Box<Pattern>),
    Cosh(Box<Pattern>),
    Tanh(Box<Pattern>),

    Asinh(Box<Pattern>),
    Acosh(Box<Pattern>),
    Atanh(Box<Pattern>),

    Arg(Box<Pattern>),
    Conj(Box<Pattern>),
    Norm(Box<Pattern>),
    Sign(Box<Pattern>),

    Real(Box<Pattern>),
    Imag(Box<Pattern>),

    Pow { base: Box<Pattern>, exp: Box<Pattern> },
    Log { base: Box<Pattern>, arg: Box<Pattern> },
    Atan2 { a: Box<Pattern>, b: Box<Pattern> },
}

pub struct Equivalency {
    from: Pattern,
    to: Pattern,
    cond: fn(Wildcard, Expr) -> bool,
}
