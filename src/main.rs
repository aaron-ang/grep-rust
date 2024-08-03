use std::env;
use std::io;
use std::process;
use std::str::Chars;

mod pattern;

use pattern::{get_patterns, Pattern};

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

fn match_substrings(
    input_line: &str,
    patterns: &[Pattern],
    start_anchor: bool,
    end_anchor: bool,
) -> bool {
    let range_start = if end_anchor {
        if input_line.len() < patterns.len() {
            return false;
        }
        // check last patterns.len() characters of input_line
        input_line.len() - patterns.len()
    } else {
        0
    };
    let range_end = if start_anchor {
        // one iteration
        range_start + 1
    } else {
        input_line.len()
    };

    'input_line: for i in range_start..range_end {
        let mut substring_iter = input_line[i..].chars();
        let mut pattern_iter = patterns.iter();
        while let Some(pattern) = pattern_iter.next() {
            match pattern {
                Pattern::Literal(l) => {
                    if !match_literal(&mut substring_iter, *l) {
                        continue 'input_line;
                    }
                }
                Pattern::Digit => {
                    if !match_digit(&mut substring_iter) {
                        continue 'input_line;
                    }
                }
                Pattern::Alphanumeric => {
                    if !match_alphanumeric(&mut substring_iter) {
                        continue 'input_line;
                    }
                }
                Pattern::Group(positive, group) => {
                    if match_group(&mut substring_iter, group) != *positive {
                        continue 'input_line;
                    }
                }
                Pattern::OneOrMore(pattern) => {
                    let mut substring_copy = String::from_iter(substring_iter);
                    let first_match = match_pattern(&substring_copy, pattern);
                    if !first_match {
                        continue 'input_line;
                    }

                    let remaining_patterns = pattern_iter.cloned().collect::<Vec<_>>();
                    loop {
                        substring_copy = substring_copy[1..].to_string();
                        if match_substrings(&substring_copy, &remaining_patterns, false, false) {
                            return true;
                        }
                        if !match_pattern(&substring_copy, pattern) {
                            break;
                        }
                    }
                    continue 'input_line;
                }
            }
        }
        return true;
    }
    false
}

fn match_pattern(input_line: &str, pattern: &Pattern) -> bool {
    let mut input_line = input_line.chars();
    match pattern {
        Pattern::Literal(l) => match_literal(&mut input_line, *l),
        Pattern::Digit => match_digit(&mut input_line),
        Pattern::Alphanumeric => match_alphanumeric(&mut input_line),
        Pattern::Group(positive, group) => match_group(&mut input_line, group) == *positive,
        p => panic!("Unexpected pattern: {:?}", p),
    }
}

fn match_literal(chars: &mut Chars, literal: char) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c == literal)
}
fn match_digit(chars: &mut Chars) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c.is_ascii_digit())
}
fn match_alphanumeric(chars: &mut Chars) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c.is_alphanumeric())
}
fn match_group(chars: &mut Chars, group: &str) -> bool {
    let c = chars.next();
    c.is_some_and(|c| group.contains(c))
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
