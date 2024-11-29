use std::{env, io, process};

use anyhow::{anyhow, bail, Result};
use colored::Colorize;

use grep_starter_rust::match_regex;

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() -> Result<()> {
    let mut args = env::args();

    let first_arg = args.nth(1);
    if first_arg.is_none() || first_arg.unwrap() != "-E" {
        bail!("Expected first argument to be '-E'");
    }

    let pattern = args
        .next()
        .ok_or_else(|| anyhow!("Expected second argument to be a pattern"))?;

    let mut input_line = String::new();
    io::stdin().read_line(&mut input_line)?;

    if let Some(group) = match_regex(&input_line, &pattern) {
        let i = input_line.find(&group).unwrap();
        let j = i + group.len();
        print!(
            "{}{}{}",
            input_line[..i].normal(),
            input_line[i..j].bright_red().bold(),
            input_line[j..].normal()
        );
        process::exit(0);
    } else {
        process::exit(1);
    }
}
