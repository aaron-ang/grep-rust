use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, IsTerminal, Write},
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};

use grep_rust::{compile_regex, find_all_regex_spans_compiled, CompiledRegex, RegexMatch};

const ANSI_BOLD_RED: &[u8] = b"\x1b[1;31m";
const ANSI_RESET: &[u8] = b"\x1b[0m";

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
        writer.write_all(ANSI_BOLD_RED)?;
        writer.write_all(&line.as_bytes()[matched.start..matched.end])?;
        writer.write_all(ANSI_RESET)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_single_ansi_highlight() {
        let mut output = Vec::new();
        write_rendered_line(
            &mut output,
            "I have 3 apples",
            "",
            true,
            &[RegexMatch { start: 7, end: 8 }],
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "I have \x1b[1;31m3\x1b[0m apples\n"
        );
    }

    #[test]
    fn writes_multiple_ansi_highlights() {
        let mut output = Vec::new();
        write_rendered_line(
            &mut output,
            "a1b2c3",
            "",
            true,
            &[
                RegexMatch { start: 1, end: 2 },
                RegexMatch { start: 3, end: 4 },
                RegexMatch { start: 5, end: 6 },
            ],
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "a\x1b[1;31m1\x1b[0mb\x1b[1;31m2\x1b[0mc\x1b[1;31m3\x1b[0m\n"
        );
    }
}
