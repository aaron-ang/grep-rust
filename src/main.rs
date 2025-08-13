use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Result};
use clap::{ArgAction, Parser};
use colored::Colorize;

use grep_starter_rust::match_regex;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Search for pattern in files
    #[arg(short = 'E')]
    pattern: String,

    /// Recursively search through directories
    #[arg(short, action = ArgAction::SetTrue)]
    recursive: bool,

    /// Files to search
    #[arg(num_args = 0..)]
    files: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let match_count = if args.files.is_empty() {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        process_lines(reader, &args.pattern, None)?
    } else {
        let file_paths = collect_files(&args.files, args.recursive)?;
        let mut total = 0;
        let show_prefix = file_paths.len() > 1;
        for file_path in file_paths {
            let file = File::open(&file_path)?;
            let reader = BufReader::new(file);
            let prefix = show_prefix.then_some(file_path.to_str().unwrap());
            total += process_lines(reader, &args.pattern, prefix)?;
        }
        total
    };

    process::exit(if match_count > 0 { 0 } else { 1 });
}

fn process_lines<R: BufRead>(reader: R, pattern: &str, filename: Option<&str>) -> Result<usize> {
    let mut match_count = 0;
    let prefix = filename.map(|s| format!("{s}:")).unwrap_or_default();
    for line in reader.lines() {
        let line = line?;
        if let Some(matched) = match_regex(&line, pattern) {
            match_count += 1;

            let output = match line.find(&matched) {
                Some(start) => {
                    let end = start + matched.len();
                    format!(
                        "{}{}{}{}",
                        prefix,
                        &line[..start],
                        &line[start..end].bright_red().bold(),
                        &line[end..]
                    )
                }
                None => format!("{prefix}{line}"),
            };

            println!("{output}");
        }
    }
    Ok(match_count)
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
