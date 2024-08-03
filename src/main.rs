use std::env;
use std::io;
use std::process;

mod r#match;
mod pattern;

use pattern::get_patterns;
use r#match::match_substrings;

fn match_regex(input_line: &str, mut regex: &str) -> bool {
    let input_line = input_line.trim();
    let start = if regex.starts_with('^') {
        regex = &regex[1..];
        true
    } else {
        false
    };
    let end = match regex.chars().last() {
        Some(c) if c == '$' => {
            regex = &regex[..regex.len() - 1];
            true
        }
        _ => false,
    };
    let patterns = get_patterns(regex);
    println!("{:?}", patterns);
    match_substrings(input_line, &patterns, start, end)
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    if env::args().nth(1).unwrap() != "-E" {
        println!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    if match_regex(&input_line, &pattern) {
        process::exit(0)
    } else {
        process::exit(1)
    }
}
