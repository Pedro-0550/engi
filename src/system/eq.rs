use crate::{expr::Expr, symbol::Symbol};

/* --------------------------------- STRUCTS -------------------------------- */

#[derive(Clone, PartialEq)]
pub struct Equation {
    lhs: Expr,
    rhs: Expr,
}

#[derive(Clone, PartialEq)]
pub enum Inequality {
    Greater,
    GreaterOrEq,
    Less,
    LessOrEq,
}

#[derive(Clone, PartialEq)]
pub struct Inequation {
    lhs: Expr,
    rhs: Expr,
    ineq: Inequality,
}

/* ---------------------------------- IMPLS --------------------------------- */

impl Inequation {
    pub fn new(lhs: Expr, rhs: Expr, ineq: Inequality) -> Self {
        Self { lhs, rhs, ineq }
    }
}

impl Equation {
    pub fn new(lhs: Expr, rhs: Expr) -> Self {
        Self { lhs, rhs }
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        let mut result = Vec::new();

        result.extend(self.lhs.symbols());
        result.extend(self.rhs.symbols());

        result.sort_unstable();
        result.dedup();

        result
    }
}

#[macro_export]
macro_rules! equation {
    (@parse [$($lhs:tt)*] = $($rhs:tt)*) => {
        $crate::system::eq::Equation::new(
            ($($lhs)*).into(),
            ($($rhs)*).into(),
        )
    };

    (@parse [$($lhs:tt)*] > $($rhs:tt)*) => {
        $crate::system::eq::Inequation::new(
            ($($lhs)*).into(),
            ($($rhs)*).into(),
            $crate::system::eq::Inequality::Greater
        )
    };

    (@parse [$($lhs:tt)*] >= $($rhs:tt)*) => {
        $crate::system::eq::Inequation::new(
            ($($lhs)*).into(),
            ($($rhs)*).into(),
            $crate::system::eq::Inequality::GreaterOrEq
        )
    };

    (@parse [$($lhs:tt)*] > $($rhs:tt)*) => {
        $crate::system::eq::Inequation::new(
            ($($lhs)*).into(),
            ($($rhs)*).into(),
            $crate::system::eq::Inequality::Greater
        )
    };

    (@parse [$($lhs:tt)*] <= $($rhs:tt)*) => {
        $crate::system::eq::Inequation::new(
            ($($lhs)*).into(),
            ($($rhs)*).into(),
            $crate::system::eq::Inequality::LessOrEq
        )
    };

    (@parse [$($lhs:tt)*] $next:tt $($rest:tt)*) => {
        equation!(@parse [$($lhs)* $next] $($rest)*)
    };

    ($($tokens:tt)*) => {
        equation!(@parse [] $($tokens)*)
    };
}

#[macro_export]
macro_rules! equations {
    (
        $(
            $($equation:tt)+
        ),+ $(,)?
    ) => {
        vec![
            $(
                $crate::equation!($($equation)+)
            ),+
        ]
    };
}
