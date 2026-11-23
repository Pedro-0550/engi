use core::slice;
use std::{
    array::{self},
    marker::PhantomData,
    ops::{Add, Deref, Div, Index, Mul, Neg, Sub},
};

use itertools::Itertools;
use ordered_float::Pow;

use crate::{
    core::{indexed::Size, util::impl_op_permutations},
    expr::Expr,
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    unit::Quantity,
};

/* Can we just take a moment to appreciate the type system
 * Like, holy **** its so good
 */

/* -------------------------------------------------------------------------- */

pub trait Stateable: Sized + Clone {
    type View<S: States>: StatefulView<S, Of = Self> = Viewless<Self>;
}

pub trait StatefulView<S: States> {
    type Of: Clone;

    fn new(states: &S::Storage<Self::Of>) -> Self;
}

#[derive(Clone, Copy, Default)]
pub struct Viewless<T>(PhantomData<T>);

impl<S: States, T: Clone> StatefulView<S> for Viewless<T> {
    type Of = T;

    fn new(_: &S::Storage<Self::Of>) -> Self {
        Self(PhantomData)
    }
}

/* -------------------------------------------------------------------------- */

pub trait States: Sized + Copy + 'static {
    const VARIANTS: &'static [Self];
    type Storage<T: Clone>: Storage<T>;

    fn idx(&self) -> usize;
    fn name(&self) -> &'static str;
}

// Why do we need this trait if its only implementor is [T; N]?
// Well in order to use the array directly we would need const_generic_exprs,
// due to the array length const parameter, but it's unfinished and has some funny business going on with
// this specific setup, where it errors with an "unconstrained const generic" even with approrpiate constraints
pub trait Storage<T>:
    IntoIterator<Item = T> + Index<usize, Output = T> + AsRef<[T]> + Clone {
    fn from_fn(f: impl FnMut(usize) -> T) -> Self;
    fn repeat(t: T) -> Self;
    fn iter<'a>(&'a self) -> slice::Iter<'a, T>;
}

impl<T, const N: usize> Storage<T> for [T; N]
where
    [T; N]: Sized,
    T: Clone,
{
    fn from_fn(f: impl FnMut(usize) -> T) -> Self {
        array::from_fn(f)
    }

    fn repeat(t: T) -> Self {
        array::repeat(t)
    }

    fn iter<'a>(&'a self) -> slice::Iter<'a, T> {
        self.as_slice().iter()
    }
}

/* -------------------------------------------------------------------------- */

#[derive(Clone)]
pub struct Stateful<T, S>
where
    T: Stateable,
    S: States, {
    states: S::Storage<T>,
    view: T::View<S>,
    _p: PhantomData<S>,
}

impl<T, S> Stateful<T, S>
where
    T: Stateable,
    S: States,
{
    pub fn new(states: S::Storage<T>) -> Self {
        Self { view: T::View::new(&states), states, _p: PhantomData }
    }

    pub fn repeat(state: T) -> Self
    where
        T: Clone, {
        let states = S::Storage::repeat(state);
        Self { view: T::View::new(&states), states, _p: PhantomData }
    }

    pub fn from_fn(mut f: impl FnMut(S) -> T) -> Self {
        let states = S::Storage::from_fn(|i| f(S::VARIANTS[i]));
        Self { view: T::View::new(&states), states, _p: PhantomData }
    }

    pub fn zip<U: Stateable>(
        self,
        rhs: Self,
        mut f: impl FnMut(T, T) -> U,
    ) -> Stateful<U, S> {
        let mut zipped = self
            .into_iter()
            .zip(rhs.into_iter())
            .map(|(a, b)| Some(f(a, b)))
            .collect_vec(); // TODO: Collect straight into array

        Stateful::<U, S>::from_fn(|s| zipped[s.idx()].take().unwrap())
    }

    pub fn map<U: Stateable>(
        self,
        mut f: impl FnMut(T) -> U,
    ) -> Stateful<U, S> {
        let mut mapped = self.into_iter().map(|x| Some(f(x))).collect_vec(); // TODO: Collect straight into array

        Stateful::<U, S>::from_fn(|s| mapped[s.idx()].take().unwrap())
    }
}

impl<T, S> Index<S> for Stateful<T, S>
where
    T: Stateable,
    S: States,
{
    type Output = T;

    fn index(&self, s: S) -> &Self::Output {
        &self.states[s.idx()]
    }
}

impl<T, S> Default for Stateful<T, S>
where
    T: Stateable + Default,
    S: States,
{
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T, S> IntoIterator for Stateful<T, S>
where
    T: Stateable,
    S: States,
{
    type Item = T;
    type IntoIter = <<S as States>::Storage<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.states.into_iter()
    }
}

impl<'a, T, S> IntoIterator for &'a Stateful<T, S>
where
    T: Stateable,
    S: States,
{
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.states.iter()
    }
}

impl<'s, S, T> Deref for Stateful<T, S>
where
    S: States,
    T: Stateable,
{
    type Target = T::View<S>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl<T, S> Stateable for Stateful<T, S>
where
    S: States,
    T: Stateable,
    T::View<S>: Clone,
{
}

/* -------------------------------------------------------------------------- */

impl<'s, S> Stateful<VariableBuilder<'s>, S>
where
    S: States,
{
    pub fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        for state in self.states.iter() {
            state.bind(expr.clone())
        }
    }

    pub fn guess(&self, qty: impl Into<Quantity>) {
        let qty = qty.into();

        for state in self.states.iter() {
            state.guess(qty.clone())
        }
    }
}

impl Stateable for VariableBuilder<'_> {}

impl<'s, S> Stateful<ConnectorBuilder<'s>, S>
where
    S: States,
{
    pub fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        for state in self.states.iter() {
            state.bind(expr.clone())
        }
    }

    pub fn guess(&self, qty: impl Into<Quantity>) {
        let qty = qty.into();

        for state in self.states.iter() {
            state.guess(qty.clone())
        }
    }
}

impl Stateable for ConnectorBuilder<'_> {}

/* -------------------------------------------------------------------------- */

impl Stateable for Variable {}
impl Stateable for Connector {}

/* -------------------------------------------------------------------------- */

impl Stateable for Expr {}
impl Stateable for &Expr {}
impl Stateable for Quantity {}
impl Stateable for Option<Quantity> {}
impl Stateable for &Quantity {}

impl Stateable for i64 {}
impl Stateable for &i64 {}

impl Stateable for f64 {}
impl Stateable for &f64 {}

impl<S: States, T> Add for Stateful<T, S>
where
    T: Add + Stateable,
    <T as Add>::Output: Stateable,
{
    type Output = Stateful<<T as Add>::Output, S>;

    fn add(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a + b)
    }
}

impl<S: States, T> Sub for Stateful<T, S>
where
    T: Sub + Stateable,
    <T as Sub>::Output: Stateable,
{
    type Output = Stateful<<T as Sub>::Output, S>;

    fn sub(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a - b)
    }
}

impl<S: States, T> Mul for Stateful<T, S>
where
    T: Mul + Stateable,
    <T as Mul>::Output: Stateable,
{
    type Output = Stateful<<T as Mul>::Output, S>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a * b)
    }
}

impl<S: States, T> Div for Stateful<T, S>
where
    T: Div + Stateable,
    <T as Div>::Output: Stateable,
{
    type Output = Stateful<<T as Div>::Output, S>;

    fn div(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a / b)
    }
}

impl<S: States, T> Pow<Self> for Stateful<T, S>
where
    T: Pow<T> + Stateable,
    <T as Pow<T>>::Output: Stateable,
{
    type Output = Stateful<<T as Pow<T>>::Output, S>;

    fn pow(self, rhs: Self) -> Self::Output {
        self.zip(rhs, |a, b| a.pow(b))
    }
}

impl<S: States, T> Neg for Stateful<T, S>
where
    T: Neg + Stateable,
    <T as Neg>::Output: Stateable,
{
    type Output = Stateful<<T as Neg>::Output, S>;

    fn neg(self) -> Self::Output {
        self.map(|x| -x)
    }
}

impl<S: States> From<&Stateful<Expr, S>> for Stateful<Expr, S> {
    fn from(value: &Stateful<Expr, S>) -> Self {
        value.clone()
    }
}

/* -------------------------------------------------------------------------- */

macro_rules! impl_stateful_expr_from_single {
    ($t:ty) => {
        impl<S> From<$t> for Stateful<Expr, S>
        where
            S: States,
        {
            fn from(value: $t) -> Self {
                Self::repeat(value.into())
            }
        }

        impl<S> From<&$t> for Stateful<Expr, S>
        where
            S: States,
        {
            fn from(value: &$t) -> Self {
                Self::repeat(value.clone().into())
            }
        }
    };
}

macro_rules! impl_stateful_expr_from_stateful {
    ($t:ty) => {
        impl<S> From<Stateful<$t, S>> for Stateful<Expr, S>
        where
            S: States,
        {
            fn from(value: Stateful<$t, S>) -> Self {
                todo!()
            }
        }

        impl<S> From<&Stateful<$t, S>> for Stateful<Expr, S>
        where
            S: States,
        {
            fn from(value: &Stateful<$t, S>) -> Self {
                todo!()
            }
        }
    };
}

impl_stateful_expr_from_single!(Quantity);
impl_stateful_expr_from_single!(i64);
impl_stateful_expr_from_single!(f64);
impl_stateful_expr_from_single!(Expr);
impl_stateful_expr_from_single!(Connector);
impl_stateful_expr_from_single!(Variable);
impl_stateful_expr_from_single!(ConnectorBuilder<'_>);
impl_stateful_expr_from_single!(VariableBuilder<'_>);

impl_stateful_expr_from_stateful!(Quantity);
impl_stateful_expr_from_stateful!(i64);
impl_stateful_expr_from_stateful!(f64);
impl_stateful_expr_from_stateful!(Connector);
impl_stateful_expr_from_stateful!(Variable);
impl_stateful_expr_from_stateful!(ConnectorBuilder<'_>);
impl_stateful_expr_from_stateful!(VariableBuilder<'_>);

impl_op_permutations! {
    params = [S],
    where = [S: States],
    types = [
        i64, Stateful<i64, S>,
        f64, Stateful<f64, S>,

        Variable, Stateful<Variable, S>,
        &Variable, &Stateful<Variable, S>,

        Connector, Stateful<Connector, S>,
        &Connector, &Stateful<Connector, S>,

        VariableBuilder<'_>, Stateful<VariableBuilder<'_>, S>,
        &VariableBuilder<'_>, &Stateful<VariableBuilder<'_>, S>,

        ConnectorBuilder<'_>, Stateful<ConnectorBuilder<'_>, S>,
        &ConnectorBuilder<'_>, &Stateful<ConnectorBuilder<'_>, S>,

        Expr, Stateful<Expr, S>,
        &Expr, &Stateful<Expr, S>,

        Quantity, Stateful<Quantity, S>,
        &Quantity, &Stateful<Quantity, S>,
    ],
    include = [
        bi(&? Stateful<Expr, S>, * except [Stateful<Expr, S>]),
        bi(&? Expr, &? Stateful<*, S>)
    ],
    into = Stateful<Expr, S>,
    out = Stateful<Expr, S>,

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
}
