use std::{
    env,
    fs::File,
    io::{self, BufRead, BufReader},
    process,
};

use anyhow::{anyhow, bail, Result};
use colored::Colorize;

use grep_starter_rust::match_regex;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);

    let flag = args.next();
    if flag.is_none() || flag.unwrap() != "-E" {
        bail!("Expected first argument to be '-E'");
    }

    let pattern = args
        .next()
        .ok_or_else(|| anyhow!("Expected second argument to be a pattern"))?;

    fn process_lines<R: BufRead>(reader: R, pattern: &str) -> Result<usize> {
        let mut match_count = 0;
        for line in reader.lines() {
            let line = line?;
            if let Some(matched) = match_regex(&line, pattern) {
                match_count += 1;
                if let Some(start) = line.find(&matched) {
                    let end = start + matched.len();
                    println!(
                        "{}{}{}",
                        line[..start].normal(),
                        line[start..end].bright_red().bold(),
                        line[end..].normal()
                    );
                } else {
                    println!("{}", line);
                }
            }
        }
        Ok(match_count)
    }

    let match_count = match args.next() {
        Some(file_name) => {
            let file = File::open(file_name)?;
            let reader = BufReader::new(file);
            process_lines(reader, &pattern)?
        }
        None => {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin.lock());
            process_lines(reader, &pattern)?
        }
    };

    process::exit(if match_count > 0 { 0 } else { 1 });
}
