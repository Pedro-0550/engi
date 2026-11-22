use core::slice;
use std::{
    array::{self, IntoIter},
    marker::PhantomData,
    mem::{self, transmute},
    ops::{Deref, Index},
};

use crate::{
    core::indexed::Size,
    expr::Expr,
    model::{
        AssembledModel, Connector, ConnectorBuilder, Interface, Model,
        Variable, VariableBuilder,
    },
    unit::Quantity,
};

#[derive(Clone)]
pub struct Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized, {
    states: [T; S::COUNT],
    view: T::View<S>,
    _p: PhantomData<S>,
}

pub trait States: Sized + Copy + 'static {
    const COUNT: usize;
    const VARIANTS: &'static [Self];
    const NAMES: [&'static str];

    fn index(&self) -> Size;
}

pub trait Stateable {
    type View<S: States>: StatefulView<S> = ();
}

pub trait StatefulView<S: States> {
    type Of;

    fn new(states: &[Self::Of]) -> Self;
}

impl<S: States> StatefulView<S> for () {
    type Of = ();

    fn new(states: &[Self::Of]) -> Self {
        ()
    }
}

/* -------------------------------------------------------------------------- */

impl<T, S> Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized,
{
    pub fn new(states: [T; S::COUNT]) -> Self {
        Self {
            view: T::View::new(unsafe {
                /* SAFETY: we know T::View::Of == T, but we just can't prove it */
                transmute(&states[..])
            }),
            states,
            _p: PhantomData,
        }
    }

    pub fn all(state: T) -> Self
    where
        T: Clone, {
        let states = array::repeat(state);
        Self {
            view: T::View::new(unsafe { transmute(&states[..]) }),
            states,
            _p: PhantomData,
        }
    }

    pub fn from_fn(mut f: impl FnMut(S) -> T) -> Self {
        let states = array::from_fn(|i| f(S::VARIANTS[i]));
        Self {
            view: T::View::new(unsafe { transmute(&states[..]) }),
            states,
            _p: PhantomData,
        }
    }
}

impl<T, S> Index<S> for Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized,
{
    type Output = T;

    fn index(&self, s: S) -> &Self::Output {
        &self.states[s.index() as usize]
    }
}

impl<T, S> Default for Stateful<T, S>
where
    T: Stateable + Default,
    S: States,
    [T; S::COUNT]: Sized,
{
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T, S> IntoIterator for Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized,
{
    type Item = T;
    type IntoIter = array::IntoIter<T, { S::COUNT }>;

    fn into_iter(self) -> Self::IntoIter {
        self.states.into_iter()
    }
}

impl<'a, T, S> IntoIterator for &'a Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized,
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
    [T; S::COUNT]: Sized,
{
    type Target = T::View<S>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

/* -------------------------------------------------------------------------- */

impl<'s, S> Stateful<VariableBuilder<'s>, S>
where
    S: States,
    [VariableBuilder<'s>; S::COUNT]: Sized,
{
    fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        for state in &self.states {
            state.bind(expr.clone())
        }
    }

    fn guess(&self, qty: impl Into<Quantity>) {
        let qty = qty.into();

        for state in &self.states {
            state.guess(qty.clone())
        }
    }
}

impl Stateable for VariableBuilder<'_> {}

impl<'s, S> Stateful<ConnectorBuilder<'s>, S>
where
    S: States,
    [ConnectorBuilder<'s>; S::COUNT]: Sized,
{
    fn bind(&self, expr: impl Into<Expr>) {
        let expr = expr.into();
        for state in &self.states {
            state.bind(expr.clone())
        }
    }

    fn guess(&self, qty: impl Into<Quantity>) {
        let qty = qty.into();

        for state in &self.states {
            state.guess(qty.clone())
        }
    }
}

impl Stateable for ConnectorBuilder<'_> {}

/* -------------------------------------------------------------------------- */

impl Stateable for Variable {}
impl Stateable for Connector {}
