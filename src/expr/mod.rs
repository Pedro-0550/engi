use std::{
    array,
    fmt::{Debug, Display, Pointer},
    hash::Hash,
    mem::{discriminant, take},
    num::NonZero,
    rc::Rc,
    sync::Arc,
};

use dashmap::mapref::one::Ref;
use derive_more::{Deref, DerefMut, From, IsVariant};
use itertools::Itertools;
use num::complex::ComplexFloat;
use ordered_float::Pow;

use crate::{
    core::{
        interned::{Handle, Interned},
        util::impl_as_variant,
        value::Value,
    },
    expr::ops::{
        Atan2, Binary, Log, Matrix, Unary, Variadic, acos, acosh, asin, atan2,
        cos, cosh, exp, ln, sin, sinh, sqrt,
    },
    simplify::{Simplify, normal::Normalize},
    symbol::{Symbol, constants::Constant},
    units::Quantity,
};

/* -------------------------------- CONSTANTS ------------------------------- */

// static NODES: Arena<Node> = Arena::new();

/* --------------------------------- MODULES -------------------------------- */

pub mod impls;
pub mod ops;

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(PartialEq, Clone, From, IsVariant, Hash, Eq)]
#[from(forward)]
pub enum Node {
    Symbol(Symbol),
    Constant(Constant),
    Quantity(Quantity),
    Variadic(Variadic),
    Unary(Unary),
    Binary(Binary),
    Matrix(Matrix),
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Domain {
    Real,
    Imag,
    Complex,
}

#[derive(Clone, Eq)]
pub struct Expr {
    node: Arc<Node>,
    hash: u64,
}

/* --------------------------------- STRUCTS -------------------------------- */

// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
// pub struct Expr(Handle<Node>);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Binding {
    pub from: Symbol,
    pub to: Expr,
}

impl Binding {
    pub fn new(from: Symbol, to: Expr) -> Self {
        Self { from, to }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Shape {
    rows: NonZero<usize>,
    cols: NonZero<usize>,
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Shaped {
    fn shape(&self) -> Shape;
}

/* ---------------------------------- IMPLS --------------------------------- */

impl_as_variant!(
    Node,
    [Symbol => Symbol,
    Quantity => Quantity,
    Constant => Constant,
    Variadic => Variadic,
    Unary => Unary,
    Binary => Binary,
    Matrix => Matrix,]
);

impl From<(usize, usize)> for Shape {
    fn from(value: (usize, usize)) -> Self {
        Self::rect(value.0, value.1)
    }
}

impl Shape {
    // SAFETY:
    // As of August 2026, 1 is not equal to 0.
    // If this changes in the future, use checked version instead.
    pub const SCALAR: Self = unsafe {
        Shape {
            cols: NonZero::<usize>::new_unchecked(1),
            rows: NonZero::<usize>::new_unchecked(1),
        }
    };

    pub fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    pub fn square(size: usize) -> Self {
        Self { rows: size.try_into().unwrap(), cols: size.try_into().unwrap() }
    }

    pub fn rect(rows: usize, cols: usize) -> Self {
        Self { rows: rows.try_into().unwrap(), cols: cols.try_into().unwrap() }
    }

    pub fn is_scalar(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() == 1
    }

    pub fn is_row(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() == 1
    }

    pub fn is_col(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() > 1
    }

    pub fn is_vec(&self) -> bool {
        (self.rows.get() > 1) ^ (self.cols.get() > 1)
    }

    pub fn is_rect(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() > 1
    }

    pub fn is_square(&self) -> bool {
        self.rows.get() > 1 && self.rows == self.rows
    }
}

impl Hash for Expr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl Expr {
    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        let Expr { node, .. } = self;

        match Arc::try_unwrap(node) {
            Ok(node) => node,
            Err(node) => (*node).clone(),
        }
    }

    /// Splits this expression `x` into `(a, b)` real and imaginary parts, such that `x = a + bi`
    pub fn realize(&self) -> [Self; 2] {
        match self.node() {
            Node::Symbol(symbol) => {
                [symbol.real().into(), symbol.imag().into()]
            }
            Node::Constant(constant) => {
                let [re, im] = constant.quantity().value().realize();
                [re.into(), im.into()]
            }
            Node::Quantity(quantity) => {
                let [re, im] = quantity.value().realize();
                [re.into(), im.into()]
            }
            Node::Variadic(variadic) => match variadic {
                Variadic::Mul(operands) => operands.iter().fold(
                    [1.into(), 0.into()],
                    |[a_re, a_im], b| {
                        let [b_re, b_im] = b.realize();

                        [
                            &a_re * &b_re - &a_im * &b_im,
                            a_re * b_im + a_im * b_re,
                        ]
                    },
                ),
                Variadic::Add(operands) => operands.iter().fold(
                    [0.into(), 0.into()],
                    |[a_re, a_im], b| {
                        let [b_re, b_im] = b.realize();

                        [a_re + b_re, a_im + b_im]
                    },
                ),
            },
            Node::Unary(unary) => {
                let [re, im] = unary.arg().realize();
                match unary {
                    Unary::Sin(_) => {
                        [sin(&re) * cosh(&im), cos(&re) * sinh(&im)]
                    }
                    Unary::Cos(_) => {
                        [cos(&re) * cosh(&im), -sin(&re) * sinh(&im)]
                    }
                    Unary::Tan(_) => {
                        let denom = cos(2 * &re) + cosh(2 * &im);
                        [sin(2 * &re) / &denom, sinh(2 * &im) / &denom]
                    }
                    Unary::Sinh(_) => {
                        [sinh(&re) * cos(&im), cosh(&re) * sin(&im)]
                    }
                    Unary::Cosh(_) => {
                        [cosh(&re) * cos(&im), sinh(&re) * sin(&im)]
                    }
                    Unary::Tanh(_) => {
                        let denom = cosh(2 * &re) + cos(2 * &im);
                        [sinh(2 * &re) / &denom, sin(2 * &im) / &denom]
                    }
                    Unary::Asin(_) => {
                        let a = sqrt((&re + 1).pow(2) + (&im).pow(2));
                        let b = sqrt((&re - 1).pow(2) + (&im).pow(2));
                        [asin((&a - &b) / 2), acosh((&a + &b) / 2)]
                    }
                    Unary::Acos(_) => {
                        let a = sqrt((&re + 1).pow(2) + (&im).pow(2));
                        let b = sqrt((&re - 1).pow(2) + (&im).pow(2));
                        [acos((&a - &b) / 2), -acosh((&a + &b) / 2)]
                    }
                    Unary::Atan(_) => [
                        atan2(2 * &re, 1 - (&re).pow(2) - (&im).pow(2)) / 2,
                        ln(((&re).pow(2) + (&im + 1).pow(2))
                            / ((&re).pow(2) + (&im - 1).pow(2)))
                            / 4,
                    ],
                    Unary::Asinh(_) => {
                        let ah = sqrt((&re).pow(2) + (&im + 1).pow(2));
                        let bh = sqrt((&re).pow(2) + (&im - 1).pow(2));
                        [acosh((&ah + &bh) / 2), asin((&ah - &bh) / 2)]
                    }
                    Unary::Acosh(_) => {
                        let a = sqrt((&re + 1).pow(2) + (&im).pow(2));
                        let b = sqrt((&re - 1).pow(2) + (&im).pow(2));
                        [acosh((&a + &b) / 2), acos((&a - &b) / 2)]
                    }
                    Unary::Atanh(_) => [
                        ln(((1 + &re).pow(2) + (&im).pow(2))
                            / ((1 - &re).pow(2) + (&im).pow(2)))
                            / 4,
                        atan2(2 * &im, 1 - (&re).pow(2) - (&im).pow(2)) / 2,
                    ],
                    Unary::Transpose(_) => todo!(),
                    Unary::Conj(_) => [re, -im],
                    Unary::Arg(_) => [atan2(im, re), 0.into()],
                    Unary::Norm(_) => [sqrt(re.pow(2) + im.pow(2)), 0.into()],
                    Unary::Real(_) => [re, 0.into()],
                    Unary::Imag(_) => [0.into(), im],
                    Unary::Det(_) => todo!(),
                }
            }
            Node::Binary(binary) => match binary {
                Binary::Pow(ops::Pow { base, exp: exponent }) => {
                    let [base_re, base_im] = base.realize();
                    let [exp_re, exp_im] = exponent.realize();

                    let u1 = ln((&base_re).pow(2) + (&base_im).pow(2)) / 2;
                    let v1 = atan2(&base_im, &base_re);

                    let r = &exp_re * &u1 - &exp_im * &v1;
                    let i = &exp_re * &v1 + &exp_im * &u1;

                    [exp(&r) * cos(&i), exp(&r) * sin(&i)]
                }
                Binary::Log(Log { base, arg }) => {
                    let [base_re, base_im] = base.realize();
                    let [arg_re, arg_im] = arg.realize();

                    let u1 = ln((&base_re).pow(2) + (&base_im).pow(2)) / 2;
                    let v1 = atan2(&base_im, &base_re);

                    let u2 = ln((&arg_re).pow(2) + (&arg_im).pow(2)) / 2;
                    let v2 = atan2(&arg_im, &arg_re);

                    let denom = (&u1).pow(2) + (&v1).pow(2);

                    [
                        (&u1 * &u2 + &v1 * &v2) / &denom,
                        (&u1 * &v2 - &u2 * &v1) / &denom,
                    ]
                }
                Binary::Atan2(Atan2 { a, b }) => {
                    let [a_re, a_im] = a.realize();
                    let [b_re, b_im] = b.realize();

                    let denom = (&b_re).pow(2) + (&b_im).pow(2);
                    let u = (&a_re * &b_re + &a_im * &b_im) / &denom;
                    let v = (&a_im * &b_re - &a_re * &b_im) / &denom;

                    [
                        atan2(2 * &u, 1 - (&u).pow(2) - (&v).pow(2)) / 2,
                        ln(((&u).pow(2) + (&v + 1).pow(2))
                            / ((&u).pow(2) + (&v - 1).pow(2)))
                            / 4,
                    ]
                }
            },
            Node::Matrix(matrix) => {
                let mut re = Matrix::zeros(matrix.rows(), matrix.cols());
                let mut im = Matrix::zeros(matrix.rows(), matrix.cols());

                for i in 0..matrix.rows().into() {
                    for j in 0..matrix.cols().into() {
                        let [re_el, im_el] = matrix[i][j].realize();
                        re[i][j] = re_el;
                        im[i][j] = im_el;
                    }
                }

                [re.into(), im.into()]
            }
        }
    }

    pub fn domain(&self) -> Domain {
        match self.node() {
            Node::Symbol(symbol) => symbol.domain(),
            Node::Constant(constant) => constant.quantity().value().domain(),
            Node::Quantity(quantity) => quantity.value().domain(),
            Node::Variadic(variadic) => match variadic {
                Variadic::Add(operands) => {
                    let mut iter = operands.iter();
                    if let Some(first) = iter.next() {
                        iter.fold(first.domain(), |acc, op| {
                            match (acc, op.domain()) {
                                (Domain::Complex, _) | (_, Domain::Complex) => {
                                    Domain::Complex
                                }
                                (Domain::Real, Domain::Imag)
                                | (Domain::Imag, Domain::Real) => {
                                    Domain::Complex
                                }
                                (Domain::Real, Domain::Real) => Domain::Real,
                                (Domain::Imag, Domain::Imag) => Domain::Imag,
                            }
                        })
                    } else {
                        Domain::Real
                    }
                }
                Variadic::Mul(operands) => {
                    let mut iter = operands.iter();
                    if let Some(first) = iter.next() {
                        iter.fold(first.domain(), |acc, op| {
                            match (acc, op.domain()) {
                                (Domain::Complex, _) | (_, Domain::Complex) => {
                                    Domain::Complex
                                }
                                (Domain::Real, Domain::Real) => Domain::Real,
                                (Domain::Imag, Domain::Imag) => Domain::Real,
                                (Domain::Real, Domain::Imag)
                                | (Domain::Imag, Domain::Real) => Domain::Imag,
                            }
                        })
                    } else {
                        Domain::Real
                    }
                }
            },
            Node::Unary(unary) => match unary {
                Unary::Sin(expr)
                | Unary::Tan(expr)
                | Unary::Sinh(expr)
                | Unary::Tanh(expr)
                | Unary::Asinh(expr)
                | Unary::Atanh(expr) => match expr.domain() {
                    Domain::Real => Domain::Real,
                    Domain::Imag => Domain::Imag,
                    Domain::Complex => Domain::Complex,
                },
                Unary::Cos(expr) | Unary::Cosh(expr) => match expr.domain() {
                    Domain::Real | Domain::Imag => Domain::Real,
                    Domain::Complex => Domain::Complex,
                },
                Unary::Asin(_) | Unary::Acos(_) | Unary::Acosh(_) => {
                    Domain::Complex
                }
                Unary::Atan(expr) => match expr.domain() {
                    Domain::Real => Domain::Real,
                    _ => Domain::Complex,
                },
                Unary::Transpose(expr) | Unary::Conj(expr) => expr.domain(),
                Unary::Arg(_)
                | Unary::Det(_)
                | Unary::Norm(_)
                | Unary::Real(_)
                | Unary::Imag(_) => Domain::Real,
            },
            Node::Binary(binary) => match binary {
                Binary::Pow(pow) => match (pow.base.domain(), pow.exp.domain())
                {
                    (Domain::Real, Domain::Real) => {
                        if let Node::Constant(c) = pow.exp.node() {
                            if c.quantity().value().is_scalar_integer() {
                                return Domain::Real;
                            }
                        }
                        Domain::Complex
                    }
                    (Domain::Imag, Domain::Real) => {
                        if let Node::Constant(c) = pow.exp.node() {
                            if let Some(n) =
                                c.quantity().value().as_scalar_integer()
                            {
                                return if n % 2 == 0 {
                                    Domain::Real
                                } else {
                                    Domain::Imag
                                };
                            }
                        }
                        Domain::Complex
                    }
                    _ => Domain::Complex,
                },
                Binary::Log(_) => Domain::Complex,
                Binary::Atan2(_) => Domain::Real,
            },
            Node::Matrix(matrix) => {
                let mut iter = matrix.elements().iter();
                if let Some(first) = iter.next() {
                    iter.fold(first.domain(), |a, b| match (a, b.domain()) {
                        (Domain::Real, Domain::Real) => Domain::Real,
                        (Domain::Real, Domain::Imag)
                        | (Domain::Imag, Domain::Real) => Domain::Complex,
                        (Domain::Imag, Domain::Imag) => Domain::Imag,
                        (_, Domain::Complex) | (Domain::Complex, _) => {
                            Domain::Complex
                        }
                    })
                } else {
                    Domain::Real
                }
            }
        }
    }

    /// Returns the total number of nodes in this expression
    pub fn size(&self) -> usize {
        match self.node() {
            Node::Symbol(_) => 1,
            Node::Constant(_) => 1,
            Node::Quantity(_) => 1,
            Node::Variadic(variadic) => {
                variadic.operands().iter().map(|x| x.size()).sum::<usize>() + 1
            }
            Node::Unary(single) => single.arg().size() + 1,
            Node::Binary(double) => {
                double.args()[0].size() + double.args()[1].size() + 1
            }
            Node::Matrix(matrix) => {
                matrix.elements().iter().map(|x| x.size()).sum::<usize>() + 1
            }
        }
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        fn symbols_inner(expr: &Expr, vec: &mut Vec<Symbol>) {
            match expr.node() {
                Node::Symbol(symbol) => vec.push(*symbol),
                Node::Quantity(quantity) => (),
                Node::Constant(_) => (),
                Node::Variadic(variadic) => {
                    for op in variadic.operands() {
                        symbols_inner(op, vec);
                    }
                }
                Node::Unary(unary) => symbols_inner(unary.arg(), vec),
                Node::Binary(binary) => {
                    symbols_inner(binary.args()[0], vec);
                    symbols_inner(binary.args()[1], vec);
                }
                Node::Matrix(matrix) => {
                    for element in matrix.elements() {
                        symbols_inner(element, vec);
                    }
                }
            }
        }

        let mut vec = Vec::new();
        symbols_inner(self, &mut vec);
        vec
    }

    pub fn substitute(&self, bindings: &[Binding]) -> Self {
        match self.node() {
            Node::Variadic(op) => op
                .with_operands(
                    op.operands()
                        .iter()
                        .map(|x| x.substitute(bindings))
                        .collect(),
                )
                .into(),
            Node::Constant(c) => c.into(),

            Node::Unary(op) => {
                op.with_arg(op.arg().substitute(bindings)).into()
            }
            Node::Quantity(qty) => qty.into(),

            Node::Binary(op) => op
                .with_args(array::from_fn(|i| {
                    op.args()[i].substitute(bindings)
                }))
                .into(),

            Node::Symbol(sym) => {
                if let Some(binding) = bindings.iter().find(|b| b.from == *sym)
                {
                    binding.to.clone()
                } else {
                    self.clone()
                }
            }

            Node::Matrix(m) => {
                Node::Matrix(m.map(|el| el.substitute(bindings))).into()
            }
        }
    }

    pub fn eval(&self, bindings: &[Binding]) -> Expr {
        match self.node() {
            Node::Constant(constant) => constant.quantity().into(),
            Node::Variadic(v) => {
                let evaled_ops: Vec<Expr> =
                    v.operands().iter().map(|op| op.eval(bindings)).collect();

                let extract_value = |expr: &Expr| {
                    let node = expr.node();
                    node.as_constant()
                        .map(|x| x.quantity().value().clone())
                        .or_else(|| {
                            node.as_quantity().map(|q| q.value().clone())
                        })
                };

                let mut const_values = Vec::new();
                let mut symbolic_ops = Vec::new();

                for op in evaled_ops {
                    if let Some(val) = extract_value(&op) {
                        const_values.push(val);
                    } else {
                        symbolic_ops.push(op);
                    }
                }

                match v {
                    Variadic::Add(_) => {
                        if let Some(folded_val) =
                            const_values.into_iter().reduce(|acc, x| acc + x)
                        {
                            if symbolic_ops.is_empty() {
                                return folded_val.into();
                            }
                            symbolic_ops.push(folded_val.into());
                        }
                        v.with_operands(symbolic_ops).normalize(true)
                    }
                    Variadic::Mul(_) => {
                        if let Some(folded_val) =
                            const_values.into_iter().reduce(|acc, x| acc * x)
                        {
                            if symbolic_ops.is_empty() {
                                return folded_val.into();
                            }
                            symbolic_ops.push(folded_val.into());
                        }
                        v.with_operands(symbolic_ops).normalize(true)
                    }
                    _ => {
                        if let Some(folded_val) =
                            const_values.into_iter().reduce(|acc, _| acc)
                        {
                            symbolic_ops.push(folded_val.into());
                        }
                        v.with_operands(symbolic_ops).normalize(true)
                    }
                }
            }
            Node::Unary(unary)
                if let arg = unary.arg().eval(bindings).node()
                    && let Some(qty) = arg
                        .as_constant()
                        .map(|x| x.quantity())
                        .or(arg.as_quantity().cloned()) =>
            {
                match unary {
                    // TODO: Should trying to evaluate dimensioned values in transcedental functions error?
                    Unary::Sin(_) => qty.value().sin().into(),
                    Unary::Cos(_) => qty.value().cos().into(),
                    Unary::Tan(_) => qty.value().tan().into(),
                    Unary::Asin(_) => qty.value().asin().into(),
                    Unary::Acos(_) => qty.value().acos().into(),
                    Unary::Atan(_) => qty.value().atan().into(),
                    Unary::Sinh(_) => qty.value().sinh().into(),
                    Unary::Cosh(_) => qty.value().cosh().into(),
                    Unary::Tanh(_) => qty.value().tanh().into(),
                    Unary::Asinh(_) => qty.value().asinh().into(),
                    Unary::Acosh(_) => qty.value().acosh().into(),
                    Unary::Atanh(_) => qty.value().atanh().into(),
                    Unary::Transpose(expr) => todo!(),
                    Unary::Conj(expr) => todo!(),
                    Unary::Arg(expr) => todo!(),
                    Unary::Det(expr) => todo!(),
                    Unary::Norm(expr) => qty.value().norm().into(),
                    Unary::Real(_) => {
                        let [re, _] = qty.value().realize();
                        re.into()
                    }
                    Unary::Imag(_) => {
                        let [_, im] = qty.value().realize();
                        im.into()
                    }
                }
            }
            Node::Binary(binary) => {
                let evaled_lhs = binary.args()[0].eval(bindings);
                let evaled_rhs = binary.args()[1].eval(bindings);

                let lhs_node = evaled_lhs.node();
                let rhs_node = evaled_rhs.node();

                let lhs_qty = lhs_node
                    .as_constant()
                    .map(|x| x.quantity())
                    .or(lhs_node.as_quantity().cloned());

                let rhs_qty = rhs_node
                    .as_constant()
                    .map(|x| x.quantity())
                    .or(rhs_node.as_quantity().cloned());

                if let (Some(l_qty), Some(r_qty)) = (lhs_qty, rhs_qty) {
                    match binary {
                        Binary::Pow(_) => {
                            l_qty.value().pow(r_qty.value()).into()
                        }
                        Binary::Log(_) => {
                            Value::from(
                                r_qty.value().as_scalar().unwrap().log(
                                    l_qty.value().as_scalar_real().unwrap(),
                                ),
                            )
                            .into()
                        }
                        Binary::Atan2(_) => l_qty
                            .value()
                            .as_scalar_real()
                            .unwrap()
                            .atan2(r_qty.value().as_scalar_real().unwrap())
                            .into(),

                        _ => binary
                            .with_args([evaled_lhs, evaled_rhs])
                            .normalize(true),
                    }
                } else {
                    binary.with_args([evaled_lhs, evaled_rhs]).normalize(true)
                }
            }
            Node::Matrix(matrix) => matrix.map(|x| x.eval(bindings)).into(),
            Node::Symbol(s) => {
                if let Some(bind) = bindings.iter().find(|x| x.from == *s) {
                    bind.to.clone()
                } else {
                    s.into()
                }
            }
            other => other.into(),
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Shaped for Expr {
    fn shape(&self) -> Shape {
        match self.node() {
            Node::Symbol(symbol) => symbol.shape(),
            Node::Quantity(v) => v.value().shape(),
            Node::Constant(c) => c.quantity().value().shape(),
            Node::Variadic(variadic) => variadic.shape(),
            Node::Unary(single) => single.shape(),
            Node::Binary(double) => double.shape(),
            Node::Matrix(matrix) => matrix.shape(),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.node() {
            Node::Quantity(qty) => <Quantity as Display>::fmt(&qty, f),
            Node::Binary(op) => <Binary as Display>::fmt(&op, f),
            Node::Unary(op) => <Unary as Display>::fmt(&op, f),
            Node::Variadic(op) => <Variadic as Display>::fmt(&op, f),
            Node::Symbol(symb) => <Symbol as Display>::fmt(&symb, f),
            Node::Constant(c) => <Constant as Display>::fmt(&c, f),
            Node::Matrix(_m) => todo!(),
        }
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.node() {
            Node::Quantity(qty) => <Quantity as Display>::fmt(&qty, f),
            Node::Binary(op) => write!(f, "{:?}", op),
            Node::Unary(op) => write!(f, "{:?}", op),
            Node::Variadic(op) => write!(f, "{:?}", op),
            Node::Symbol(symb) => <Symbol as Display>::fmt(&symb, f),
            Node::Constant(c) => <Constant as Display>::fmt(&c, f),
            Node::Matrix(_m) => todo!(),
        }
    }
}
