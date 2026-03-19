use std::{
    io::{self, IsTerminal},
    process,
};

use anyhow::Result;
use clap::{Parser, ValueEnum};

use grep_rust::{
    compile_regex,
    search_runner::{run_search, SearchConfig},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ColorMode {
    Always,
    Auto,
    Never,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'o')]
    only_matching: bool,

    #[arg(long, value_enum, default_value_t = ColorMode::Never)]
    color: ColorMode,

    #[arg(short = 'E', allow_hyphen_values = true, value_name = "pattern")]
    pattern: String,

    #[arg(short = 'r')]
    recursive: bool,

    #[arg(short = 'j', long, default_value_t = default_thread_count(), value_name = "threads")]
    threads: usize,

    #[arg(value_name = "file")]
    files: Vec<std::path::PathBuf>,
}

fn default_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let compiled = compile_regex(&args.pattern);
    let use_color = match args.color {
        ColorMode::Always => true,
        ColorMode::Auto => io::stdout().is_terminal(),
        ColorMode::Never => false,
    };

    let match_count = run_search(
        &args.files,
        args.recursive,
        &compiled,
        SearchConfig {
            only_matching: args.only_matching,
            use_color,
            show_prefix: false,
            threads: args.threads.max(1),
        },
    )?;

    process::exit(i32::from(match_count == 0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_thread_count_flag() {
        let args = Args::parse_from(["grep-rust", "-j", "8", "-E", "foo", "file.txt"]);
        assert_eq!(args.threads, 8);
    }

    #[test]
    fn default_thread_count_is_non_zero() {
        assert!(default_thread_count() >= 1);
    }
}
