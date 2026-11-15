use std::marker::PhantomData;

use crate::{core::indexed::Size, expr::Expr, model::VariableBuilder};

pub struct Stateful<T, I>
where
    T: Stateable,
    I: States,
    [T; I::VARIANTS]: Sized, {
    states: [T; I::VARIANTS],
    _p: PhantomData<I>,
}

pub trait States {
    const VARIANTS: usize;

    fn name(&self) -> &'static str;
    fn index(&self) -> Size;
}

pub trait Stateable {}

impl<I> Stateful<Expr, I>
where
    I: States,
    [Expr; I::VARIANTS]: Sized,
{
}

impl Stateable for Expr {}
impl Stateable for VariableBuilder<'_> {}
