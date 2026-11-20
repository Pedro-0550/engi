use std::{
    array,
    marker::PhantomData,
    mem,
    ops::{Deref, Index},
};

use crate::{
    core::indexed::Size,
    expr::Expr,
    model::{AssembledModel, Model, Variable, VariableBuilder},
    unit::Quantity,
};

pub struct Stateful<T, S>
where
    T: Stateable,
    S: States,
    [T; S::COUNT]: Sized, {
    states: [T; S::COUNT],
    view: T::View,
    _p: PhantomData<S>,
}

pub trait States {
    const COUNT: usize;
    const VARIANTS: [Self; Self::COUNT];
    const NAMES: [&'static str];

    fn index(&self) -> Size;
}

pub trait Stateable {
    type View: StatefulView = ();
}

pub trait StatefulView<S: States> {
    type Of;

    fn new(states: &[Self::Of]) -> Self;
}

impl<S: States> StatefulView<S: States> for () {
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
    fn new(states: [T; S::COUNT]) -> Self {
        Self { states, view: T::View::new(&states), _p: PhantomData }
    }

    fn all(state: T) -> Self
    where
        T: Clone,
        T::View: From<()>, {
        let states = array::repeat(state);
        Self { states, view: T::View::new(&states), _p: PhantomData }
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

/* -------------------------------------------------------------------------- */

impl Stateable for Variable {}

/* -------------------------------------------------------------------------- */

impl<'s, S, M> Deref for Stateful<M, S>
where
    S: States,
    M: Model,
    [M; S::COUNT]: Sized,
{
    type Target = M::StatefulView<S>;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl<M: Model> Stateable for M {
    type View<S: States> = M::StatefulView<S>;
}
