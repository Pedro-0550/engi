use ordered_float::Pow;

use crate::simplify::pattern::{
    Condition, Rule, Wildcard, acos, acosh, asin, asinh, atan, atanh, cos,
    cosh, ln, log, sin, sinh, tan, tanh,
};

macro_rules! wildcards {
    ($($var:ident),* $(,)?) => {
        $(
            const $var: Wildcard = Wildcard::from(${index()} as usize);
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
                        f: ( |$ctx| { $b }),
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
                        f: ( |$ctx| { $b }),
                    }
                ),*
            )?
        ]) }, Rule { from: $to.into(), to: $from.into(), conds: Box::new([
            $(
                $(
                    Condition {
                        over: Box::new($over),
                        f: ( |$ctx| { $b }),
                    }
                ),*
            )?
        ]) }]
    };
}

wildcards!(x, y, a, b);

pub fn algebraic() -> Vec<Rule> {
    [
        rule!({ x + y } -> { y + x }),
        rule!({ x + 0 } -> { x }),

        rule!({ x - x } -> { 0 }),
        rule!({ -(-x) } <-> { x }),

        /* -------------------------------------------------------------------------- */

        rule!({ x * 0 } -> { 0 }),

        rule!({ x * 1 } <-> { x }),

        rule!({ x * y } -> { y * x } when [
            |ctx| {
                let x_s = ctx.shape(x);
                let y_s = ctx.shape(y);
                x_s.is_scalar() || y_s.is_scalar()
            } over [x, y]
        ]),
        rule!({ x * (a + b) } <-> { x * a + x * b }),
        rule!({ (a + b) + x } <-> { a + (b + x) }),
        rule!({ (a * b) * x } <-> { a * (b * x) }),

        /* -------------------------------------------------------------------------- */

        rule!({ x / x } -> { 1 }),
        rule!({ x / 1 } <-> { x }),

        /* -------------------------------------------------------------------------- */

        rule!({ x.pow(0) } -> { 1 }),
        rule!({ x.pow(1) } <-> { x }),
        rule!({ (x + y).pow(2) } <-> { x.pow(2) + 2 * x * y + y.pow(2) }),

        rule!({ x.pow(a) * x.pow(b) } <-> { x.pow(a + b) } when [
            |ctx| {
                let x_d = ctx.domain(x);

                x_d.is_real()
            } over [x],
            |ctx| {
                let a_d = ctx.domain(a);

                a_d.is_real()
            } over [a],
            |ctx| {
                let b_d = ctx.domain(b);

                b_d.is_real()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                let a_d = ctx.domain(a);
                let b_d = ctx.domain(b);

                x_d.re.is_pos() || (
                    !x_d.re.has_zero()
                    && a_d.is_integer()
                    && b_d.is_integer()
                )
            } over [x, a, b]
        ]),

        rule!({ x.pow(a).pow(b) } <-> { x.pow(a * b) } when [
            |ctx| {
                let x_d = ctx.domain(x);

                x_d.is_real()
            } over [x],
            |ctx| {
                let a_d = ctx.domain(a);

                a_d.is_real()
            } over [a],
            |ctx| {
                let b_d = ctx.domain(b);

                b_d.is_real()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                let a_d = ctx.domain(a);
                let b_d = ctx.domain(b);

                x_d.re.is_pos() || (
                    !x_d.re.has_zero()
                    && a_d.is_integer()
                    && b_d.is_integer()
                )
            } over [x, a, b]
        ]),

        /* -------------------------------------------------------------------------- */

        rule!({ log(b, 1) } -> { 0 } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero()
            } over [b]
        ]),

        rule!({ log(b, b) } -> { 1 } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero()
            } over [b]
        ]),

        rule!({ log(b, x.pow(a)) } -> { a * log(b, x) } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero() && b_d.is_real()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                !x_d.has_zero() && x_d.is_real() && x_d.re.is_pos()
            } over [x],
            |ctx| {
                let a_d = ctx.domain(a);
                a_d.is_real()
            } over [a]
        ]),

        rule!({ log(b, x * a) } <-> { log(b, x) + log(b, a) } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero() && b_d.is_real()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                !x_d.has_zero() && x_d.is_real() && x_d.re.is_pos()
            } over [x],
            |ctx| {
                let a_d = ctx.domain(a);
                !a_d.has_zero() && a_d.is_real() && a_d.re.is_pos()
            } over [a]
        ]),

        rule!({ log(b, x / a) } <-> { log(b, x) - log(b, a) } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero() && b_d.is_real()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                !x_d.has_zero() && x_d.is_real() && x_d.re.is_pos()
            } over [x],
            |ctx| {
                let a_d = ctx.domain(a);
                !a_d.has_zero() && a_d.is_real() && a_d.re.is_pos()
            } over [a]
        ]),

        rule!({ log(b, b.pow(x)) } -> { x } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero() && b_d.is_real() && b_d.re.is_pos()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                x_d.is_real()
            } over [x]
        ]),

        rule!({ b.pow(log(b, x)) } -> { x } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                !x_d.has_zero()
            } over [x]
        ]),

        rule!({ log(b, x) } <-> { ln(x)/ln(b) } when [
            |ctx| {
                let b_d = ctx.domain(b);
                !b_d.has_zero()
            } over [b],
            |ctx| {
                let x_d = ctx.domain(x);
                !x_d.has_zero()
            } over [x]
        ]),
    ]
    .concat()
}

pub fn trig() -> Vec<Rule> {
    [
        rule!({ cos(x).pow(2) + sin(x).pow(2) } -> { 1 }),
        rule!({ sin(x) / cos(x) } <-> { tan(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sin(-x) } <-> { -sin(x) }),
        rule!({ cos(-x) } <-> { cos(x) }),
        rule!({ tan(-x) } <-> { -tan(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sin(asin(x)) } -> { x }),
        rule!({ cos(acos(x)) } -> { x }),
        rule!({ tan(atan(x)) } -> { x }),
        /* -------------------------------------------------------------------------- */
        rule!({ cosh(x).pow(2) - sinh(x).pow(2) } -> { 1 }),
        rule!({ sinh(x) / cosh(x) } <-> { tanh(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sinh(-x) } <-> { -sinh(x) }),
        rule!({ cosh(-x) } <-> { cosh(x) }),
        rule!({ tanh(-x) } <-> { -tanh(x) }),
        /* -------------------------------------------------------------------------- */
        rule!({ sinh(asinh(x)) } -> { x }),
        rule!({ cosh(acosh(x)) } -> { x }),
        rule!({ tanh(atanh(x)) } -> { x }),

    ]
    .concat()
}
