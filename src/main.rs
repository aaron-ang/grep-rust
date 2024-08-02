use std::env;
use std::io;
use std::process;

fn match_pattern(input_line: &str, pattern: &str) -> bool {
    if pattern.chars().count() == 1 {
        return input_line.contains(pattern);
    } else if pattern.contains("\\d") {
        return input_line.contains(|c: char| c.is_ascii_digit());
    } else if pattern.contains("\\w") {
        return input_line.contains(|c: char| c.is_alphanumeric());
    } else if pattern.starts_with("[^") && pattern.ends_with(']') {
        let neg_chars = &pattern[2..pattern.len() - 1];
        return input_line.chars().all(|c| !neg_chars.contains(c));
    } else if pattern.starts_with('[') && pattern.ends_with(']') {
        let pos_chars = &pattern[1..pattern.len() - 1];
        return input_line.chars().any(|c| pos_chars.contains(c));
    } else {
        panic!("Unhandled pattern: {}", pattern)
    }
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
