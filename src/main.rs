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

    if start {
        if patterns
            .iter()
            .all(|p| match_substring(&mut input_line, p, &mut groups, &mut current_group))
        {
            if !end || input_line.peek().is_none() {
                Some(current_group)
            } else {
                None
            }
        } else {
            None
        }
    } else {
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
            if input_line.next().is_none() {
                return None;
            }
            current_group.clear();
            groups.clear();
        }
    }
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    if env::args().nth(1).unwrap() != "-E" {
        eprintln!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    if let Some(group) = match_regex(&input_line, &pattern) {
        println!("{}", group.bright_red().bold());
        process::exit(0)
    } else {
        process::exit(1)
    }
}
