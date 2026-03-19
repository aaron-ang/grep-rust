use std::hint::black_box;

use grep_rust::{compile_regex, find_all_regex_spans_compiled, CompiledRegex};

pub fn bench_compile(pattern: &str) {
    black_box(compile_regex(pattern));
}

pub fn bench_find_all_with_compiled(compiled: &CompiledRegex, input: &str) {
    black_box(find_all_regex_spans_compiled(input, compiled));
}
