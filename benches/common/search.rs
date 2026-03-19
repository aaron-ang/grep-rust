use std::{
    hint::black_box,
    io,
    path::{Path, PathBuf},
};

use grep_rust::{
    search_runner::{run_search_to_writer, search_line_by_line, search_text_content, SearchConfig},
    CompiledRegex,
};

pub fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("bench")
        .join(relative)
}

pub fn read_fixture(relative: &str) -> String {
    let path = fixture_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn default_search_config(threads: usize) -> SearchConfig {
    SearchConfig {
        only_matching: false,
        use_color: false,
        show_prefix: false,
        threads,
    }
}

pub fn bench_search_text_with_compiled(compiled: &CompiledRegex, input: &str) {
    let mut output = Vec::new();
    black_box(search_text_content(
        input,
        compiled,
        &default_search_config(1),
        "",
        &mut output,
    ));
    black_box(output.len());
}

pub fn bench_search_text_line_by_line_with_compiled(compiled: &CompiledRegex, input: &str) {
    let mut output = Vec::new();
    black_box(search_line_by_line(
        input,
        compiled,
        &default_search_config(1),
        "",
        &mut output,
    ));
    black_box(output.len());
}

pub fn bench_file_search_with_compiled(
    compiled: &CompiledRegex,
    files: &[PathBuf],
    recursive: bool,
    threads: usize,
) {
    let mut sink = io::sink();
    black_box(
        run_search_to_writer(
            &mut sink,
            files,
            recursive,
            compiled,
            default_search_config(threads),
        )
        .expect("file search benchmark should succeed"),
    );
}
