use std::env;
use std::io;
use std::process;

mod r#match;
mod pattern;

use colored::Colorize;
use pattern::parse;
use r#match::match_substring;

fn match_regex(input_line: &str, regex: &str) -> Option<String> {
    let mut input_line = input_line.trim().chars().peekable();
    let (patterns, start, end) = parse(regex);
    let mut groups = vec![];
    let mut current_group = String::new();

    loop {
        let mut input_start = input_line.clone();
        if patterns
            .iter()
            .all(|p| match_substring(&mut input_start, p, &mut groups, &mut current_group))
        {
            if !end || input_start.peek().is_none() {
                return Some(current_group);
            } else {
                return None;
            }
        }
        if start {
            // first and only match failed
            return None;
        }
        if input_line.next().is_none() {
            return None;
        }
        current_group.clear();
        groups.clear();
    }
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    let first_arg = env::args().nth(1);
    if first_arg.is_none() || first_arg.unwrap() != "-E" {
        eprintln!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2);
    if pattern.is_none() {
        eprintln!("Expected second argument to be a pattern");
        process::exit(1);
    }

    let mut input_line = String::new();
    io::stdin().read_line(&mut input_line).unwrap();

    if let Some(group) = match_regex(&input_line, &pattern.unwrap()) {
        let i = input_line.find(&group).unwrap();
        let j = i + group.len();
        print!(
            "{}{}{}",
            input_line[..i].normal(),
            input_line[i..j].bright_red().bold(),
            input_line[j..].normal()
        );
        process::exit(0)
    } else {
        process::exit(1)
    }
}
