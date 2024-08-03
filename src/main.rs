use std::env;
use std::io;
use std::iter::Peekable;
use std::process;
use std::str::Chars;

#[derive(Debug)]
enum Pattern {
    Literal(char),
    Digit,
    Alphanumeric,
    Group(bool, String),
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
        1
    } else {
        input_line.len()
    };

    'input_line: for i in range_start..range_end {
        let mut substring = input_line[i..].chars();
        for pattern in patterns {
            match pattern {
                Pattern::Literal(l) => {
                    if !match_literal(&mut substring, *l) {
                        continue 'input_line;
                    }
                }
                Pattern::Digit => {
                    if !match_digit(&mut substring) {
                        continue 'input_line;
                    }
                }
                Pattern::Alphanumeric => {
                    if !match_alphanumeric(&mut substring) {
                        continue 'input_line;
                    }
                }
                Pattern::Group(positive, group) => {
                    if match_group(&mut substring, group) != *positive {
                        continue 'input_line;
                    }
                }
            }
        }
        return true;
    }
    false
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

fn match_pattern(input_line: &str, mut pattern: &str) -> bool {
    let input_line = input_line.trim();
    let start = if pattern.starts_with('^') {
        pattern = &pattern[1..];
        true
    } else {
        false
    };
    let end = match pattern.chars().last() {
        Some(c) if c == '$' => {
            pattern = &pattern[..pattern.len() - 1];
            true
        }
        _ => false,
    };
    let patterns = get_patterns(pattern);
    match_substrings(input_line, &patterns, start, end)
}

fn get_patterns(pattern: &str) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    let mut chars = pattern.chars().peekable();

    loop {
        let c = chars.next();
        if c.is_none() {
            break;
        }
        let pattern = match c.unwrap() {
            '\\' => {
                let c = chars.next();
                if c.is_none() {
                    panic!("Expected character after '\\'");
                }
                match c.unwrap() {
                    'd' => Pattern::Digit,
                    'w' => Pattern::Alphanumeric,
                    '\\' => Pattern::Literal('\\'),
                    unknown => panic!("Unknown special character: {}", unknown),
                }
            }
            '[' => {
                let (is_positive, group) = get_group_pattern(&mut chars);
                Pattern::Group(is_positive, group)
            }
            l => Pattern::Literal(l),
        };
        patterns.push(pattern);
    }

    patterns
}

fn get_group_pattern(chars: &mut Peekable<Chars>) -> (bool, String) {
    let mut is_positive = true;
    let mut group = String::new();

    if chars.peek() == Some(&'^') {
        is_positive = false;
        chars.next();
    }

    while chars.peek() != Some(&']') {
        let c = chars.next();
        if c.is_none() {
            panic!("Expected ']' after group");
        }
        group.push(c.unwrap());
    }
    chars.next();

    (is_positive, group)
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

    if match_pattern(&input_line, &pattern) {
        process::exit(0)
    } else {
        process::exit(1)
    }
}
