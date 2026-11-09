use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use engi::unit::defs::{K, MHz, V, eV, mV, mi};
use ordered_float::Pow;

fn normalize(c: &mut Criterion) {
    c.bench_function("normalize_5", |b| {
        let unit = mV * MHz.sqrt() / K * eV * mi;
        let qty = 50 * unit;
        b.iter_batched(
            || qty.clone(),
            |qty| qty.normalize(),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(unit, normalize);
criterion_main!(unit);
