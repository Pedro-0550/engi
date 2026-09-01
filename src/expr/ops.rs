use std::{
    cmp::Ordering,
    fmt::{self, Display, Pointer, Write},
    iter::once,
    num::NonZero,
    ops::Index,
    rc::Rc,
};

use derive_more::IsVariant;
use itertools::Itertools;
use num::complex::ComplexFloat;

use crate::{
    core::util::{impl_as_variant, to_superscript},
    expr::{Expr, Node, Shape, Shaped},
    simplify::separate_consts,
    symbol::constants::e,
    units::{Quantity, Unit},
};

#[derive(PartialEq, Clone, Debug, IsVariant, Hash, Eq)]
pub enum Variadic {
    Add(Vec<Expr>),
    Mul(Vec<Expr>),
}

#[derive(PartialEq, Clone, Debug, IsVariant, Hash, Eq)]
pub enum Unary {
    Sin(Expr),
    Cos(Expr),
    Tan(Expr),

    Asin(Expr),
    Acos(Expr),
    Atan(Expr),

    Sinh(Expr),
    Cosh(Expr),
    Tanh(Expr),

    Asinh(Expr),
    Acosh(Expr),
    Atanh(Expr),

    Transpose(Expr),
    Conj(Expr),
    Arg(Expr),
    Det(Expr),
    Norm(Expr),

    Real(Expr),
    Imag(Expr),
}

#[derive(PartialEq, Clone, Debug, Hash, Eq)]
pub struct Pow {
    pub base: Expr,
    pub exp: Expr,
}

#[derive(PartialEq, Clone, Debug, Hash, Eq)]
pub struct Log {
    pub base: Expr,
    pub arg: Expr,
}

#[derive(PartialEq, Clone, Debug, Hash, Eq)]
pub struct Atan2 {
    pub a: Expr,
    pub b: Expr,
}

#[derive(PartialEq, Clone, Debug, IsVariant, Hash, Eq)]
pub enum Binary {
    Pow(Pow),
    Log(Log),
    Atan2(Atan2),
}

/// Row-major matrix type
#[derive(PartialEq, Clone, Debug, Hash, Eq)]
pub struct Matrix {
    shape: Shape,
    elements: Vec<Expr>,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(
    Binary,
    [
        Pow => Pow,
        Log => Log,
        Atan2 => Atan2,
    ]
);

impl Matrix {
    /// Returns (rows, cols) for this matrix
    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn rows(&self) -> NonZero<usize> {
        self.shape.rows
    }

    pub fn cols(&self) -> NonZero<usize> {
        self.shape.cols
    }

    pub fn elements(&self) -> &[Expr] {
        &self.elements
    }

    pub fn map(&self, f: impl FnMut(&Expr) -> Expr) -> Matrix {
        Matrix {
            shape: self.shape,
            elements: self.elements.iter().map(f).collect(),
        }
    }
}

impl Index<usize> for Matrix {
    type Output = [Expr];

    fn index(&self, row: usize) -> &Self::Output {
        let start = row * self.shape.cols.get();
        let end = start + self.shape.cols.get();
        &self.elements[start..end]
    }
}

impl Variadic {
    pub fn with_operands(&self, ops: Vec<Expr>) -> Self {
        match self {
            Variadic::Add(_) => Variadic::Add(ops),
            Variadic::Mul(_) => Variadic::Mul(ops),
        }
    }

    pub fn operands(&self) -> &Vec<Expr> {
        match self {
            Variadic::Add(ops) => ops,
            Variadic::Mul(ops) => ops,
        }
    }

    pub fn operands_mut(&mut self) -> &mut Vec<Expr> {
        match self {
            Variadic::Add(ops) => ops,
            Variadic::Mul(ops) => ops,
        }
    }

    pub fn into_operands(self) -> Vec<Expr> {
        match self {
            Variadic::Add(ops) => ops,
            Variadic::Mul(ops) => ops,
        }
    }
}

impl Shaped for Variadic {
    fn shape(&self) -> Shape {
        match self {
            Variadic::Add(exprs) => exprs.first().unwrap().shape(),
            Variadic::Mul(exprs) => {
                exprs.iter().fold(Shape::SCALAR, |acc, term| {
                    let b = term.shape();

                    // Special case: dot product, vec * vec, but same direction only
                    if acc == b && acc.is_vec() {
                        acc
                    } else {
                        Shape { rows: acc.rows, cols: b.cols }
                    }
                })
            }
        }
    }
}

impl Unary {
    pub fn with_arg(&self, arg: Expr) -> Self {
        match self {
            Unary::Sin(_) => Unary::Sin(arg),
            Unary::Cos(_) => Unary::Cos(arg),
            Unary::Tan(_) => Unary::Tan(arg),
            Unary::Asin(_) => Unary::Asin(arg),
            Unary::Acos(_) => Unary::Acos(arg),
            Unary::Atan(_) => Unary::Atan(arg),
            Unary::Sinh(_) => Unary::Sinh(arg),
            Unary::Cosh(_) => Unary::Cosh(arg),
            Unary::Tanh(_) => Unary::Tanh(arg),
            Unary::Asinh(_) => Unary::Asinh(arg),
            Unary::Acosh(_) => Unary::Acosh(arg),
            Unary::Atanh(_) => Unary::Atanh(arg),
            Unary::Transpose(_) => Unary::Transpose(arg),
            Unary::Conj(_) => Unary::Conj(arg),
            Unary::Arg(_) => Unary::Arg(arg),
            Unary::Det(_) => Unary::Det(arg),
            Unary::Norm(_) => Unary::Norm(arg),
            Unary::Real(_) => Unary::Real(arg),
            Unary::Imag(_) => Unary::Imag(arg),
        }
    }

    pub fn into_arg(self) -> Expr {
        match self {
            Unary::Sin(arg) => arg,
            Unary::Cos(arg) => arg,
            Unary::Tan(arg) => arg,
            Unary::Asin(arg) => arg,
            Unary::Acos(arg) => arg,
            Unary::Atan(arg) => arg,
            Unary::Sinh(arg) => arg,
            Unary::Cosh(arg) => arg,
            Unary::Tanh(arg) => arg,
            Unary::Asinh(arg) => arg,
            Unary::Acosh(arg) => arg,
            Unary::Atanh(arg) => arg,
            Unary::Transpose(arg) => arg,
            Unary::Conj(arg) => arg,
            Unary::Arg(arg) => arg,
            Unary::Det(arg) => arg,
            Unary::Norm(arg) => arg,
            Unary::Real(arg) => arg,
            Unary::Imag(arg) => arg,
        }
    }

    pub fn arg(&self) -> &Expr {
        match self {
            Unary::Sin(arg) => arg,
            Unary::Cos(arg) => arg,
            Unary::Tan(arg) => arg,
            Unary::Asin(arg) => arg,
            Unary::Acos(arg) => arg,
            Unary::Atan(arg) => arg,
            Unary::Sinh(arg) => arg,
            Unary::Cosh(arg) => arg,
            Unary::Tanh(arg) => arg,
            Unary::Asinh(arg) => arg,
            Unary::Acosh(arg) => arg,
            Unary::Atanh(arg) => arg,
            Unary::Transpose(arg) => arg,
            Unary::Conj(arg) => arg,
            Unary::Arg(arg) => arg,
            Unary::Det(arg) => arg,
            Unary::Norm(arg) => arg,
            Unary::Real(arg) => arg,
            Unary::Imag(arg) => arg,
        }
    }

    pub fn arg_mut(&mut self) -> &mut Expr {
        match self {
            Unary::Sin(arg) => arg,
            Unary::Cos(arg) => arg,
            Unary::Tan(arg) => arg,
            Unary::Asin(arg) => arg,
            Unary::Acos(arg) => arg,
            Unary::Atan(arg) => arg,
            Unary::Sinh(arg) => arg,
            Unary::Cosh(arg) => arg,
            Unary::Tanh(arg) => arg,
            Unary::Asinh(arg) => arg,
            Unary::Acosh(arg) => arg,
            Unary::Atanh(arg) => arg,
            Unary::Transpose(arg) => arg,
            Unary::Conj(arg) => arg,
            Unary::Arg(arg) => arg,
            Unary::Det(arg) => arg,
            Unary::Norm(arg) => arg,
            Unary::Real(arg) => arg,
            Unary::Imag(arg) => arg,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Unary::Sin(_) => "sin",
            Unary::Cos(_) => "cos",
            Unary::Tan(_) => "tan",
            Unary::Asin(_) => "asin",
            Unary::Acos(_) => "acos",
            Unary::Atan(_) => "atan",
            Unary::Sinh(_) => "sinh",
            Unary::Cosh(_) => "cosh",
            Unary::Tanh(_) => "tanh",
            Unary::Asinh(_) => "asinh",
            Unary::Acosh(_) => "acosh",
            Unary::Atanh(_) => "atanh",
            Unary::Transpose(_) => "transpose",
            Unary::Conj(_) => "conj",
            Unary::Arg(_) => "arg",
            Unary::Det(_) => "det",
            Unary::Norm(_) => "norm",
            Unary::Real(arg) => "real",
            Unary::Imag(arg) => "imag",
        }
    }
}

impl Shaped for Unary {
    fn shape(&self) -> Shape {
        match self {
            Self::Transpose(expr) => expr.shape().transpose(),
            Self::Det(_) | Self::Norm(_) => Shape::SCALAR,
            _ => self.arg().shape(),
        }
    }
}

impl Binary {
    pub fn with_args(&self, args: [Expr; 2]) -> Self {
        let [a, b] = args;
        match self {
            Binary::Atan2(Atan2 { .. }) => Binary::Atan2(Atan2 { a, b }),
            Binary::Log(Log { .. }) => Binary::Log(Log { base: a, arg: b }),
            Binary::Pow(Pow { .. }) => Binary::Pow(Pow { base: a, exp: b }),
        }
    }

    pub fn into_args(self) -> [Expr; 2] {
        match self {
            Binary::Atan2(Atan2 { a, b }) => [a, b],
            Binary::Log(Log { base, arg }) => [base, arg],
            Binary::Pow(Pow { base, exp }) => [base, exp],
        }
    }

    pub fn args(&self) -> [&Expr; 2] {
        match self {
            Binary::Atan2(Atan2 { a, b }) => [a, b],
            Binary::Log(Log { base, arg }) => [base, arg],
            Binary::Pow(Pow { base, exp }) => [base, exp],
        }
    }
}

impl Shaped for Binary {
    fn shape(&self) -> Shape {
        match self {
            Binary::Pow(Pow { base, exp }) => exp.shape(),
            Binary::Log(Log { base, arg }) => arg.shape(),
            Binary::Atan2(Atan2 { a, b }) => Shape::SCALAR,
        }
    }
}

impl Display for Unary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unary::Transpose(expr) => {
                let parenthesize = matches!(
                    expr.node(),
                    Node::Binary(Binary::Pow { .. }) | Node::Variadic(_)
                );

                write_enclosed(expr, f, parenthesize)
            }
            _ => {
                f.write_str(self.name())?;
                write_enclosed(self.arg(), f, true)
            }
        }
    }
}

impl Display for Binary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Binary::Pow(Pow { base, exp }) => {
                let parenthesize_base = !matches!(
                    base.node(),
                    Node::Symbol(_)
                        | Node::Const(_)
                        | Node::Unary(_)
                        | Node::Binary(
                            Binary::Log { .. } | Binary::Atan2 { .. }
                        )
                );
                let parenthesize_exp =
                    !matches!(exp.node(), Node::Symbol(_) | Node::Const(_));

                write_enclosed(base, f, parenthesize_base)?;

                if let Node::Const(x) = exp.node()
                    && x.unit() == Unit::Unitless
                    && let value = x.value()
                    && value.is_integer()
                {
                    f.write_str(&to_superscript(value.re as i32))?;
                } else {
                    f.write_str("^")?;

                    write_enclosed(exp, f, parenthesize_exp)?;
                }

                Ok(())
            }
            Binary::Log(Log { base, arg }) => {
                if *base == e.into() {
                    f.write_str("ln")?;
                    write_enclosed(arg, f, true)
                } else {
                    f.write_str("log(")?;
                    base.fmt(f)?;
                    f.write_str(", ")?;
                    arg.fmt(f)?;
                    f.write_str(")")
                }
            }
            Binary::Atan2(Atan2 { a, b }) => {
                f.write_str("atan2(")?;
                a.fmt(f)?;
                f.write_str(", ")?;
                b.fmt(f)?;
                f.write_str(")")
            }
        }
    }
}

impl Display for Variadic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variadic::Add(terms) => {
                for (i, term) in terms.iter().enumerate() {
                    if let Node::Variadic(Variadic::Mul(terms)) = term.node()
                        && let (mut consts, exprs) =
                            separate_consts(terms.iter().cloned())
                        && let Ok(coef) = consts.by_ref().exactly_one()
                        && coef.value().is_real()
                        && coef.value().re < 0.0
                    {
                        if i > 0 {
                            f.write_str(" - ")?;
                        } else {
                            f.write_str("-")?;
                        }

                        Variadic::Mul(
                            once(Expr::from(coef.value().abs() * coef.unit()))
                                .chain(exprs)
                                .collect(),
                        )
                        .fmt(f)?;
                    } else {
                        if i > 0 {
                            f.write_str(" + ")?;
                        }

                        term.fmt(f)?;
                    }
                }
            }
            Variadic::Mul(terms) => {
                fn joinable(expr: &Expr) -> bool {
                    matches!(
                        expr.node(),
                        Node::Symbol(_)
                            | Node::Const(_)
                            | Node::Binary(Binary::Pow { .. })
                    )
                }

                fn sort_terms<'a>(terms: &Vec<&'a Expr>) -> Vec<&'a Expr> {
                    let (mut mat_part, mut scalar_part): (
                        Vec<&Expr>,
                        Vec<&Expr>,
                    ) = terms.iter().partition(|x| x.shape().is_rect());

                    scalar_part.sort_by(|a, b| {
                        if matches!(a.node(), Node::Const(_)) {
                            return Ordering::Less;
                        }

                        match (joinable(a), joinable(b)) {
                            (true, false) => Ordering::Less,
                            (false, true) => Ordering::Greater,
                            _ => Ordering::Equal,
                        }
                    });

                    scalar_part.append(&mut mat_part);

                    scalar_part
                }

                let (denom, num): (Vec<&Expr>, Vec<&Expr>) =
                    terms.iter().partition(|expr| {
                        expr.node()
                            .as_binary()
                            .and_then(|bin| bin.as_pow())
                            .and_then(|pow| {
                                pow.exp
                                    .node()
                                    .as_const()
                                    .and_then(|qty| qty.value().as_real())
                            })
                            .is_some_and(|exp| exp < 0.0)
                    });

                let (denom, num) = (sort_terms(&denom), sort_terms(&num));

                for (i, term) in num.iter().enumerate() {
                    let parenthesize =
                        term.node().as_variadic().is_some_and(|v| v.is_add());
                    // matches!(term.node(), Node::Variadic(Variadic::Add(_)));

                    if i > 0
                        && num
                            .get(i - 1)
                            .map(|x| !joinable(x))
                            .unwrap_or_default()
                        && joinable(term)
                    {
                        f.write_char('·')?;
                    }

                    write_enclosed(term, f, parenthesize)?;

                    if num.get(i + 1).map(|x| !joinable(x)).unwrap_or_default()
                    {
                        f.write_char('·')?;
                    }

                    // f.write_char('·')?;
                }

                if !num.is_empty() && !denom.is_empty() {
                    f.write_str(" / ")?;
                }

                let parenthesize_denom = denom.len() > 1
                    || denom
                        .first()
                        .map(|x| x.node().is_variadic())
                        .unwrap_or_default();

                if parenthesize_denom {
                    f.write_char('(')?;
                }

                for (i, mut term) in
                    denom.iter().by_ref().cloned().cloned().enumerate()
                {
                    if !joinable(&term) && i != 0 {
                        f.write_char('·')?;
                    }

                    if !num.is_empty() {
                        let new_term = match term.node() {
                            Node::Binary(Binary::Pow(Pow { base, exp })) => {
                                let exp = match exp.node() {
                                    Node::Const(qty) => qty,
                                    _ => unreachable!(
                                        "Expression must be a Pow with negative const exp in order to be on the denominator"
                                    ),
                                };

                                if exp.value().re == -1.0 {
                                    base.clone()
                                } else {
                                    Binary::Pow(Pow {
                                        base: base.clone(),
                                        exp: (exp.value().abs() * exp.unit())
                                            .into(),
                                    })
                                    .into()
                                }
                            }
                            _ => unreachable!(
                                "Expression must be a Pow with negative const exp in order to be on the denominator"
                            ),
                        };
                        term = new_term;
                    }

                    let parenthesize =
                        matches!(term.node(), Node::Variadic(Variadic::Add(_)));

                    write_enclosed(term, f, parenthesize)?;

                    if denom
                        .get(i + 1)
                        .map(|x| !joinable(x))
                        .unwrap_or_default()
                    {
                        f.write_char('·')?;
                    }
                }

                if parenthesize_denom {
                    f.write_char(')')?;
                }
            }
        }
        Ok(())
    }
}

/* -------------------------------- FUNCTIONS ------------------------------- */

fn write_enclosed(
    obj: impl Display,
    f: &mut std::fmt::Formatter<'_>,
    parenthesize: bool,
) -> fmt::Result {
    if parenthesize {
        f.write_str("(")?;
    }
    obj.fmt(f)?;
    if parenthesize {
        f.write_str(")")?;
    }

    Ok(())
}

macro_rules! impl_single_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Expr>) -> Expr {
            let expr = x.into();
            let shape = expr.shape();

            assert!(
                shape.is_square() || shape.is_scalar(),
                "Matrix-valued {} is only defined for square matrices",
                $name
            );

            Unary::$variant(expr).into()
        }
    };
}

impl_single_fn!(sin, Sin, "sine");
impl_single_fn!(cos, Cos, "cosine");
impl_single_fn!(tan, Tan, "tangent");

impl_single_fn!(asin, Asin, "inverse sine");
impl_single_fn!(acos, Acos, "inverse cosine");
impl_single_fn!(atan, Atan, "inverse tangent");

impl_single_fn!(sinh, Sinh, "hyperbolic sine");
impl_single_fn!(cosh, Cosh, "hyperbolic cosine");
impl_single_fn!(tanh, Tanh, "hyperbolic tangent");

impl_single_fn!(asinh, Asinh, "inverse hyperbolic sine");
impl_single_fn!(acosh, Acosh, "inverse hyperbolic cosine");
impl_single_fn!(atanh, Atanh, "inverse hyperbolic tangent");

impl_single_fn!(real, Real, "real component of z");
impl_single_fn!(imag, Imag, "imaginary component of z");

/* -------------------------------------------------------------------------- */

pub fn log(base: impl Into<Expr>, x: impl Into<Expr>) -> Expr {
    let base = base.into();
    let x = x.into();

    assert!(
        base.shape().is_scalar(),
        "Logarithm is only defined for scalar bases"
    );

    assert!(
        x.shape().is_square() || x.shape().is_scalar(),
        "Matrix-valued logarithm is only defined for square matrices"
    );

    Binary::Log(Log { base: base.into(), arg: x.into() }).into()
}

pub fn ln(x: impl Into<Expr>) -> Expr {
    log(e, x)
}

pub fn exp(x: impl Into<Expr>) -> Expr {
    pow(e, x)
}

/* -------------------------------------------------------------------------- */

pub fn sqrt(x: impl Into<Expr>) -> Expr {
    pow(x.into(), 1 / 2)
}

pub fn cbrt(x: impl Into<Expr>) -> Expr {
    pow(x.into(), 1 / 3)
}

pub fn qtrt(x: impl Into<Expr>) -> Expr {
    pow(x.into(), 1 / 4)
}

pub fn pow(base: impl Into<Expr>, exp: impl Into<Expr>) -> Expr {
    let base = base.into();
    let exp = exp.into();

    assert!(
        base.shape().is_square() || base.shape().is_scalar(),
        "Only square matrices can be raised to a power"
    );

    assert!(
        exp.shape().is_square() || exp.shape().is_scalar(),
        "Only square matrices can be an exponent"
    );

    assert!(
        !(base.shape().is_square() && exp.shape().is_square()),
        "Cannot raise a matrix to the power of another matrix yet"
    );

    Binary::Pow(Pow { base, exp }).into()
}
