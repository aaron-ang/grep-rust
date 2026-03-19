#[path = "common/engine.rs"]
mod common;

use criterion::{criterion_group, criterion_main, Criterion};

fn compile_regex_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("compile_regex");
    for (name, pattern) in [
        ("literal_prefix", "matched_line_[0123456789]+"),
        (
            "dense_alternation",
            "message=(matched_line|ordinary_line)_[0123456789]+",
        ),
        ("broad_wildcard", "^.+ .+ .+$"),
        ("backref_word_repeat", r"(\w+) and \1"),
        ("backref_multiple", r"(\w+)-(\d+) and \1-\2"),
        ("backref_group_replay", r"^((\w+)-(\d+)) and \1$"),
    ] {
        group.bench_function(name, |b| b.iter(|| common::bench_compile(pattern)));
    }
    group.finish();
}

fn find_all_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_all_regex_spans_compiled");
    for (name, pattern, input) in [
        (
            "literal_prefix",
            "matched_line_[0123456789]+",
            "log=000010 level=INFO user=user_000010 code=0010 message=matched_line_000010",
        ),
        (
            "dense_alternation",
            "message=(matched_line|ordinary_line)_[0123456789]+",
            "log=000011 level=DEBUG user=user_000011 code=0011 message=ordinary_line_000011",
        ),
        ("broad_wildcard", "^.+ .+ .+$", "horse cow sheep"),
        (
            "backref_word_repeat",
            r"(\w+) and \1",
            "token123 and token123",
        ),
        (
            "backref_multiple",
            r"(\w+)-(\d+) and \1-\2",
            "token123-123456 and token123-123456",
        ),
        (
            "backref_group_replay",
            r"^((\w+)-(\d+)) and \1$",
            "token123-123456 and token123-123456",
        ),
    ] {
        let compiled = grep_rust::compile_regex(pattern);
        group.bench_function(name, |b| {
            b.iter(|| common::bench_find_all_with_compiled(&compiled, input))
        });
    }
    group.finish();
}

criterion_group!(engine_benches, compile_regex_benches, find_all_benches);
criterion_main!(engine_benches);
