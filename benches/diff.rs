use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engi::{
    diff::Differentiable,
    expr::ops::{cos, cosh, ln, log, sin, sinh, tan},
    simplify::{Simplify, SimplifyContext, normal::Normalize},
    symbol::Symbol,
    symbols,
};
use ordered_float::Pow;

fn small_expr(c: &mut Criterion) {
    c.bench_function("partial of small expr", |b| {
        symbols!(x, y);

        let f_of_xy = ((x.pow(2) + y) * sin(x * y) * ln(x / y)).normalize(true);

        b.iter(|| black_box(f_of_xy.diff(x)))
    });
}

fn large_expr(c: &mut Criterion) {
    c.bench_function("partial of large expr", |b| {
        symbols!(x, y);

        let f_of_xy = ((x.pow(3) + 2.0 * x * y + y.pow(2) + 1.0)
            * sin(x * y + x.pow(2))
            * cos(y.pow(2) + x)
            * ln((x.pow(2) + y.pow(2) + 1.0) / (x + y))
            + ((x + 1.0).pow(y)) * sinh(x * y) * cosh(x.pow(2) - y)
            + (x.pow(2) * y + x * y.pow(2) + 1.0)
                * log(x + y, x.pow(2) + y + 1.0)
                * tan(x * y))
        .normalize(true);

        b.iter(|| black_box(f_of_xy.diff(x)))
    });
}

criterion_group!(diff, large_expr, small_expr);
criterion_main!(diff);
