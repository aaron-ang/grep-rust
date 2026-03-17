use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, IsTerminal, Write},
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use colored::{control, Colorize};

use grep_rust::{compile_regex, find_all_regex_spans_compiled, CompiledRegex, RegexMatch};

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
    let compiled = compile_regex(&args.pattern);
    let use_color = match args.color {
        ColorMode::Always => true,
        ColorMode::Auto => io::stdout().is_terminal(),
        ColorMode::Never => false,
    };
    control::set_override(use_color);
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    let match_count = if args.files.is_empty() {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        process_lines(
            reader,
            &mut writer,
            &compiled,
            None,
            args.only_matching,
            use_color,
        )?
    } else {
        let file_paths = collect_files(&args.files, args.recursive)?;
        let mut total = 0;
        let show_prefix = file_paths.len() > 1;
        for file_path in file_paths {
            let file = File::open(&file_path)?;
            let reader = BufReader::new(file);
            let prefix = show_prefix.then_some(file_path.to_str().unwrap());
            total += process_lines(
                reader,
                &mut writer,
                &compiled,
                prefix,
                args.only_matching,
                use_color,
            )?;
        }
        total
    };
    writer.flush()?;

    process::exit(if match_count > 0 { 0 } else { 1 });
}

fn process_lines<R: BufRead, W: Write>(
    reader: R,
    writer: &mut W,
    pattern: &CompiledRegex,
    filename: Option<&str>,
    only_matching: bool,
    use_color: bool,
) -> Result<usize> {
    let mut match_count = 0;
    let prefix = filename.map(|s| format!("{s}:")).unwrap_or_default();
    for line in reader.lines() {
        let line = line?;
        let matches = find_all_regex_spans_compiled(&line, pattern);
        if only_matching {
            match_count += matches.len();
            for matched in matches {
                writer.write_all(prefix.as_bytes())?;
                writer.write_all(&line.as_bytes()[matched.start..matched.end])?;
                writer.write_all(b"\n")?;
            }
        } else {
            if !matches.is_empty() {
                match_count += 1;
                write_rendered_line(writer, &line, &prefix, use_color, &matches)?;
            }
        }
    }
    Ok(match_count)
}

fn write_rendered_line<W: Write>(
    writer: &mut W,
    line: &str,
    prefix: &str,
    use_color: bool,
    matches: &[RegexMatch],
) -> io::Result<()> {
    if matches.is_empty() || !use_color {
        writer.write_all(prefix.as_bytes())?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        return Ok(());
    }

    writer.write_all(prefix.as_bytes())?;

    let mut last = 0;
    for matched in matches {
        writer.write_all(&line.as_bytes()[last..matched.start])?;
        writer.write_all(
            line[matched.start..matched.end]
                .red()
                .bold()
                .to_string()
                .as_bytes(),
        )?;
        last = matched.end;
    }
    writer.write_all(&line.as_bytes()[last..])?;
    writer.write_all(b"\n")
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
