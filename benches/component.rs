//! Construction and clone benchmarks for [`TextComponent`].

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use text_components::format::Color;
use text_components::{Modifier, Style, TextComponent};

#[expect(
    clippy::missing_const_for_fn,
    reason = "`Style` is only conditionally const"
)]
fn leaf() -> TextComponent {
    TextComponent::plain("No permission")
        .color(Color::Red)
        .bold(true)
        .italic(false)
}

fn tree() -> TextComponent {
    TextComponent::plain("<")
        .add_child(TextComponent::plain("Notch").color(Color::Aqua))
        .add_child("> ")
        .add_child(TextComponent::plain("hello world"))
}

const RECIPIENTS: usize = 100;

fn construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("construct");
    group.bench_function("leaf", |b| b.iter(|| black_box(leaf())));
    group.bench_function("tree", |b| b.iter(|| black_box(tree())));
    group.finish();
}

fn clone(c: &mut Criterion) {
    let leaf = leaf();
    let tree = tree();

    let mut group = c.benchmark_group("clone");
    group.bench_function("leaf", |b| b.iter(|| black_box(leaf.clone())));
    group.bench_function("tree", |b| b.iter(|| black_box(tree.clone())));
    group.finish();
}

fn broadcast(c: &mut Criterion) {
    let leaf = leaf();
    let tree = tree();

    let mut group = c.benchmark_group("broadcast");
    group.throughput(criterion::Throughput::Elements(RECIPIENTS as u64));
    group.bench_function("leaf", |b| {
        b.iter_batched(
            || Vec::with_capacity(RECIPIENTS),
            |mut sink| {
                for _ in 0..RECIPIENTS {
                    sink.push(leaf.clone());
                }
                sink
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("tree", |b| {
        b.iter_batched(
            || Vec::with_capacity(RECIPIENTS),
            |mut sink| {
                for _ in 0..RECIPIENTS {
                    sink.push(tree.clone());
                }
                sink
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, construction, clone, broadcast);
criterion_main!(benches);
