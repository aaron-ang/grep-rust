use std::{hint::black_box, io, path::PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use grep_rust::{
    compile_regex,
    search_runner::{run_search_to_writer, search_text_content, SearchConfig},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum WorkloadCase {
    LiteralPrefix,
    DenseAlternation,
    RecursiveTree,
    BackrefWordRepeat,
    BackrefMultiple,
    BackrefGroupReplay,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_enum)]
    case: WorkloadCase,

    #[arg(long, default_value_t = 50)]
    iters: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let total = match args.case {
        WorkloadCase::LiteralPrefix => {
            run_text_workload("data.txt", "matched_line_[0123456789]+", args.iters)?
        }
        WorkloadCase::DenseAlternation => run_text_workload(
            "data.txt",
            "message=(matched_line|ordinary_line)_[0123456789]+",
            args.iters,
        )?,
        WorkloadCase::RecursiveTree => run_recursive_workload(args.iters)?,
        WorkloadCase::BackrefWordRepeat => {
            run_text_workload("backref.txt", r"(\w+) and \1", args.iters)?
        }
        WorkloadCase::BackrefMultiple => {
            run_text_workload("backref.txt", r"(\w+)-(\d+) and \1-\2", args.iters)?
        }
        WorkloadCase::BackrefGroupReplay => {
            run_text_workload("backref.txt", r"^((\w+)-(\d+)) and \1$", args.iters)?
        }
    };
    black_box(total);
    Ok(())
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("bench")
}

fn read_fixture(name: &str) -> Result<String> {
    let path = fixtures_dir().join(name);
    std::fs::read_to_string(&path)
        .map_err(Into::into)
        .and_then(|text| {
            if text.is_empty() {
                bail!(
                    "{} is empty; regenerate fixtures with `uv run scripts/gen-bench-data.sh`",
                    path.display()
                );
            }
            Ok(text)
        })
}

fn run_text_workload(fixture_name: &str, pattern: &str, iters: usize) -> Result<usize> {
    let text = read_fixture(fixture_name)?;
    let compiled = compile_regex(pattern);
    let config = SearchConfig {
        only_matching: false,
        use_color: false,
        show_prefix: false,
        threads: 1,
    };

    let mut total = 0usize;
    for _ in 0..iters {
        let mut output = Vec::new();
        total += search_text_content(&text, &compiled, &config, "", &mut output);
        black_box(output.len());
    }
    Ok(total)
}

fn run_recursive_workload(iters: usize) -> Result<usize> {
    let tree = fixtures_dir().join("tree");
    if !tree.is_dir() {
        bail!(
            "{} is missing; regenerate fixtures with `uv run scripts/gen-bench-data.sh`",
            tree.display()
        );
    }

    let compiled = compile_regex("matched_line_[0123456789]+");
    let config = SearchConfig {
        only_matching: false,
        use_color: false,
        show_prefix: false,
        threads: 4,
    };
    let files = [tree];
    let mut total = 0usize;
    for _ in 0..iters {
        total += run_search_to_writer(&mut io::sink(), &files, true, &compiled, config)?;
    }
    Ok(total)
}
