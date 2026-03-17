use std::{
    fs::File,
    io::{self, BufRead, BufReader, IsTerminal},
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use colored::{control, Colorize};

use grep_rust::{find_all_regex, find_all_regex_spans, RegexMatch};

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

    #[arg(value_name = "file")]
    files: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let use_color = match args.color {
        ColorMode::Always => true,
        ColorMode::Auto => io::stdout().is_terminal(),
        ColorMode::Never => false,
    };
    control::set_override(use_color);

    let match_count = if args.files.is_empty() {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        process_lines(reader, &args.pattern, None, args.only_matching, use_color)?
    } else {
        let file_paths = collect_files(&args.files, args.recursive)?;
        let mut total = 0;
        let show_prefix = file_paths.len() > 1;
        for file_path in file_paths {
            let file = File::open(&file_path)?;
            let reader = BufReader::new(file);
            let prefix = show_prefix.then_some(file_path.to_str().unwrap());
            total += process_lines(reader, &args.pattern, prefix, args.only_matching, use_color)?;
        }
        total
    };

    process::exit(if match_count > 0 { 0 } else { 1 });
}

fn process_lines<R: BufRead>(
    reader: R,
    pattern: &str,
    filename: Option<&str>,
    only_matching: bool,
    use_color: bool,
) -> Result<usize> {
    let mut match_count = 0;
    let prefix = filename.map(|s| format!("{s}:")).unwrap_or_default();
    for line in reader.lines() {
        let line = line?;
        if only_matching {
            let matches = find_all_regex(&line, pattern);
            match_count += matches.len();
            for matched in matches {
                println!("{prefix}{matched}");
            }
        } else {
            let matches = find_all_regex_spans(&line, pattern);
            if !matches.is_empty() {
                match_count += 1;
                println!("{}", render_matches(&line, &prefix, use_color, &matches));
            }
        }
    }
    Ok(match_count)
}

fn render_matches(line: &str, prefix: &str, use_color: bool, matches: &[RegexMatch]) -> String {
    if matches.is_empty() || !use_color {
        return format!("{prefix}{line}");
    }

    let mut output = String::with_capacity(prefix.len() + line.len());
    output.push_str(prefix);

    let mut last = 0;
    for matched in matches {
        let end = matched.start + matched.text.len();
        output.push_str(&line[last..matched.start]);
        output.push_str(&matched.text.red().bold().to_string());
        last = end;
    }
    output.push_str(&line[last..]);
    output
}

fn collect_files(inputs: &[PathBuf], recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in inputs {
        let meta = input.metadata()?;
        if meta.is_file() {
            files.push(input.clone());
        } else if meta.is_dir() {
            if !recursive {
                bail!("{}: Is a directory", input.to_string_lossy());
            }
            files.extend(collect_dir(input)?);
        }
    }
    Ok(files)
}

fn collect_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            files.extend(collect_dir(&path)?);
        } else if meta.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}
