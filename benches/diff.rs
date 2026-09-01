use std::hint::black_box;

use cas_solve::{
    diff::Differentiable,
    expr::ops::{cos, cosh, ln, log, sin, sinh, tan},
    simplify::{Simplify, SimplifyContext},
    symbol::Symbol,
    symbols,
};
use criterion::{Criterion, criterion_group, criterion_main};

fn small_expr(c: &mut Criterion) {
    c.bench_function("partial of small expr", |b| {
        symbols!(x, y);

        let f_of_xy = (((x ^ 2) + y) * sin(x * y) * ln(x / y)).normalize();

        b.iter(|| black_box(f_of_xy.diff(x)))
    });
}

fn large_expr(c: &mut Criterion) {
    c.bench_function("partial of large expr", |b| {
        symbols!(x, y);

        let f_of_xy = (((x ^ 3) + 2.0 * x * y + (y ^ 2) + 1.0)
            * sin(x * y + x ^ 2)
            * cos((y ^ 2) + x)
            * ln(((x ^ 2) + (y ^ 2) + 1.0) / (x + y))
            + ((x + 1.0) ^ y) * sinh(x * y) * cosh((x ^ 2) - y)
            + ((x ^ 2) * y + x * (y ^ 2) + 1.0)
                * log(x + y, (x ^ 2) + y + 1.0)
                * tan(x * y))
        .normalize();

        b.iter(|| black_box(f_of_xy.diff(x)))
    });
}

criterion_group!(diff, large_expr, small_expr);
criterion_main!(diff);
