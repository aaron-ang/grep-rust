#[path = "common/search.rs"]
mod common;

use criterion::{criterion_group, criterion_main, Criterion};

fn search_pipeline_benches(c: &mut Criterion) {
    let data_path = common::fixture_path("data.txt");
    let tree_path = common::fixture_path("tree");
    let data_text = common::read_fixture("data.txt");
    let literal_compiled = grep_rs::compile_regex("matched_line_[0123456789]+");
    let alternation_compiled =
        grep_rs::compile_regex("message=(matched_line|ordinary_line)_[0123456789]+");

    let mut group = c.benchmark_group("search_pipeline");
    group.bench_function("serial_file_search_literal_prefix", |b| {
        b.iter(|| {
            common::bench_file_search_with_compiled(
                &literal_compiled,
                std::slice::from_ref(&data_path),
                false,
                1,
            )
        })
    });
    group.bench_function("recursive_tree_serial", |b| {
        b.iter(|| {
            common::bench_file_search_with_compiled(
                &literal_compiled,
                std::slice::from_ref(&tree_path),
                true,
                1,
            )
        })
    });
    group.bench_function("recursive_tree_parallel", |b| {
        b.iter(|| {
            common::bench_file_search_with_compiled(
                &literal_compiled,
                std::slice::from_ref(&tree_path),
                true,
                4,
            )
        })
    });
    group.bench_function("candidate_line_dense_alternation", |b| {
        b.iter(|| common::bench_search_text_with_compiled(&alternation_compiled, &data_text))
    });
    group.bench_function("line_by_line_dense_alternation", |b| {
        b.iter(|| {
            common::bench_search_text_line_by_line_with_compiled(&alternation_compiled, &data_text)
        })
    });
    group.finish();
}

criterion_group!(search_benches, search_pipeline_benches);
criterion_main!(search_benches);
