use std::{
    env,
    fs::File,
    io::{self, BufRead, BufReader},
    process,
};

use anyhow::{anyhow, bail, Result};
use colored::Colorize;

use grep_starter_rust::match_regex;

// Usage: echo <input_text> | your_program.sh -E <pattern> <file_name>
fn main() -> Result<()> {
    let mut args = env::args().skip(1);

    let flag = args.next();
    if flag.is_none() || flag.unwrap() != "-E" {
        bail!("Expected first argument to be '-E'");
    }

    let pattern = args
        .next()
        .ok_or_else(|| anyhow!("Expected second argument to be a pattern"))?;

    let mut input_line = String::new();
    match args.next() {
        Some(file_name) => {
            let file = File::open(file_name)?;
            BufReader::new(file).read_line(&mut input_line)?;
        }
        None => {
            io::stdin().read_line(&mut input_line)?;
        }
    }

    if let Some(matched) = match_regex(&input_line, &pattern) {
        if let Some(start) = input_line.find(&matched) {
            let end = start + matched.len();
            print!(
                "{}{}{}",
                input_line[..start].normal(),
                input_line[start..end].bright_red().bold(),
                input_line[end..].normal()
            );
            process::exit(0);
        }
    }
    process::exit(1);
}
