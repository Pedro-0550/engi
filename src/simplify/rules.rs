use std::rc::Rc;

use ordered_float::Pow;

use crate::simplify::pattern::{
    Condition, Rule, Wildcard, acos, acosh, asin, asinh, atan, atanh, cos,
    cosh, ln, log, sin, sinh, tan, tanh,
};

macro_rules! wildcards {
    ($($var:ident),* $(,)?) => {
        $(
            let $var: Wildcard = Wildcard(${index()});
        )*
    };
}

macro_rules! rule {
    ($from:block -> $to:block $(when [
        $(
             |$ctx:ident| $b:block over $over:expr
        ),+
    ])?) => {
        vec![Rule { from: $from.into(), to: $to.into(), conds: Box::new([
            $(
                $(
                    Condition {
                        over: Box::new($over),
                        f: Rc::new(move |$ctx| { $b }),
                    }
                ),*
            )?
        ]) }]
    };

    ($from:block <-> $to:block $(when [
        $(
             |$ctx:ident| $b:block over $over:expr
        ),+
    ])?) => {
        vec![Rule { from: $from.into(), to: $to.into(), conds: Box::new([
            $(
                $(
                    Condition {
                        over: Box::new($over),
                        f: Rc::new(move |$ctx| { $b }),
                    }
                ),*
            )?
        ]) }, Rule { from: $to.into(), to: $from.into(), conds: Box::new([
            $(
                $(
                    Condition {
                        over: Box::new($over),
                        f: Rc::new(move |$ctx| { $b }),
                    }
                ),*
            )?
        ]) }]
    };
}

fn algebraic() -> Vec<Rule> {
    wildcards!(x, y, a, b);

    [
        rule!({ x + y } -> { y + x }),
        rule!({ x + 0 } -> { x }),
        rule!({ x - x } -> { 0 }),

        /* -------------------------------------------------------------------------- */

        rule!({ x * 0 } -> { 0 }),
        rule!({ x * 1 } <-> { x }),
        rule!({ x * y } -> { y * x } when [
            |ctx| {
                let x = ctx.shape(x);
                let y = ctx.shape(y);
                x.is_scalar() || y.is_scalar()
            } over [x, y]
        ]),
        rule!({ x * (a + b) } <-> { x * a + x * b }),

        /* -------------------------------------------------------------------------- */

        rule!({ x / x } -> { 1 }),
        rule!({ x / 1 } <-> { x }),

        /* -------------------------------------------------------------------------- */

        rule!({ x.pow(0) } -> { 1 }),
        rule!({ x.pow(1) } <-> { x }),
        rule!({ (x + y).pow(2) } <-> { x.pow(2) + 2 * x * y + y.pow(2) }),

        rule!({ x.pow(a) * x.pow(b) } <-> { x.pow(a + b) } when [
            |ctx| {
                let x = ctx.domain(x);

                x.is_real()
            } over [x],
            |ctx| {
                let a = ctx.domain(a);

                a.is_real()
            } over [a],
            |ctx| {
                let b = ctx.domain(b);

                b.is_real()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                let a = ctx.domain(a);
                let b = ctx.domain(b);

                x.re.is_pos() || (
                    !x.re.has_zero()
                    && a.is_integer()
                    && b.is_integer()
                )
            } over [x, a, b]
        ]),

        rule!({ x.pow(a).pow(b) } <-> { x.pow(a * b) } when [
            |ctx| {
                let x = ctx.domain(x);

                x.is_real()
            } over [x],
            |ctx| {
                let a = ctx.domain(a);

                a.is_real()
            } over [a],
            |ctx| {
                let b = ctx.domain(b);

                b.is_real()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                let a = ctx.domain(a);
                let b = ctx.domain(b);

                x.re.is_pos() || (
                    !x.re.has_zero()
                    && a.is_integer()
                    && b.is_integer()
                )
            } over [x, a, b]
        ]),

        /* -------------------------------------------------------------------------- */

        rule!({ log(b, 1) } -> { 0 } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero()
            } over [b]
        ]),

        rule!({ log(b, b) } -> { 1 } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero()
            } over [b]
        ]),

        rule!({ log(b, x.pow(a)) } -> { a * log(b, x) } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero() && b.is_real()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                !x.has_zero() && x.is_real() && x.re.is_pos()
            } over [x],
            |ctx| {
                let a = ctx.domain(a);
                a.is_real()
            } over [a]
        ]),

        rule!({ log(b, x * a) } <-> { log(b, x) + log(b, a) } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero() && b.is_real()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                !x.has_zero() && x.is_real() && x.re.is_pos()
            } over [x],
            |ctx| {
                let a = ctx.domain(a);
                !a.has_zero() && a.is_real() && a.re.is_pos()
            } over [a]
        ]),

        rule!({ log(b, x / a) } <-> { log(b, x) - log(b, a) } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero() && b.is_real()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                !x.has_zero() && x.is_real() && x.re.is_pos()
            } over [x],
            |ctx| {
                let a = ctx.domain(a);
                !a.has_zero() && a.is_real() && a.re.is_pos()
            } over [a]
        ]),

        rule!({ log(b, b.pow(x)) } -> { x } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero() && b.is_real() && b.re.is_pos()
                } over [b],
            |ctx| {
                let x = ctx.domain(x);
                x.is_real()
            } over [x]
        ]),

        rule!({ b.pow(log(b, x)) } -> { x } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                !x.has_zero()
            } over [x]
        ]),

        rule!({ log(b, x) } <-> { ln(x)/ln(b) } when [
            |ctx| {
                let b = ctx.domain(b);
                !b.has_zero()
            } over [b],
            |ctx| {
                let x = ctx.domain(x);
                !x.has_zero()
            } over [x]
        ]),
    ]
    .concat()
}

fn trig() -> Vec<Rule> {
    wildcards!(x);

    [
        rule!({ cos(x).pow(2) + sin(x).pow(2) } -> { 1 }),
        rule!({ sin(x) / cos(x) } <-> { tan(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sin(-x) } <-> { -sin(x) }),
        rule!({ cos(-x) } <-> { cos(x) }),
        rule!({ tan(-x) } <-> { -tan(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sin(asin(x)) } <-> { x }),
        rule!({ cos(acos(x)) } <-> { x }),
        rule!({ tan(atan(x)) } <-> { x }),
        /* -------------------------------------------------------------------------- */
        rule!({ cosh(x).pow(2) - sinh(x).pow(2) } -> { 1 }),
        rule!({ sinh(x) / cosh(x) } <-> { tanh(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sinh(-x) } <-> { -sinh(x) }),
        rule!({ cosh(-x) } <-> { cosh(x) }),
        rule!({ tanh(-x) } <-> { -tanh(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sinh(asinh(x)) } <-> { x }),
        rule!({ cosh(acosh(x)) } <-> { x }),
        rule!({ tanh(atanh(x)) } <-> { x }),

    ]
    .concat()
}
