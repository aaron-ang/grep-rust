use std::env;
use std::io;
use std::iter::Peekable;
use std::process;
use std::str::Chars;

enum Pattern {
    Literal(char),
    Digit,
    Alphanumeric,
    Group(bool, String),
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

fn match_pattern(input_line: &str, pattern: &str) -> bool {
    let input_line = input_line.trim();
    let patterns = get_patterns(pattern);
    'input_line: for i in 0..input_line.len() {
        let mut input = input_line[i..].chars();
        for pattern in &patterns {
            match pattern {
                Pattern::Literal(l) => {
                    if !match_literal(&mut input, *l) {
                        continue 'input_line;
                    }
                }
                Pattern::Digit => {
                    if !match_digit(&mut input) {
                        continue 'input_line;
                    }
                }
                Pattern::Alphanumeric => {
                    if !match_alphanumeric(&mut input) {
                        continue 'input_line;
                    }
                }
                Pattern::Group(positive, group) => {
                    if match_group(&mut input, group) != *positive {
                        continue 'input_line;
                    }
                }
            }
        }
        return true;
    }
    false
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
