use std::{iter::Peekable, str::Chars};

use crate::pattern::Pattern;

pub fn match_substrings(
    input_line: &str,
    patterns: &[Pattern],
    start_anchor: bool,
    end_anchor: bool,
) -> bool {
    let range_end = if start_anchor { 0 } else { input_line.len() };
    'input_line: for i in 0..=range_end {
        let mut substring_iter = input_line[i..].chars().peekable();
        let mut pattern_iter = patterns.iter();
        while let Some(pattern) = pattern_iter.next() {
            match pattern {
                Pattern::Literal(_)
                | Pattern::Digit
                | Pattern::Alphanumeric
                | Pattern::Wildcard => {
                    if !match_pattern(&mut substring_iter, pattern) {
                        continue 'input_line;
                    }
                }
                Pattern::Group(positive, group) => {
                    if substring_iter.peek().is_none() {
                        continue 'input_line;
                    }
                    if match_group(&mut substring_iter, group) != *positive {
                        continue 'input_line;
                    }
                }
                Pattern::OneOrMore(pattern) => {
                    if !match_pattern(&mut substring_iter, pattern) {
                        continue 'input_line; // zero match: go to next input substring
                    }
                    let remaining_patterns = pattern_iter.cloned().collect::<Vec<_>>();
                    let mut substring = String::from_iter(substring_iter);
                    // incrementally match `pattern` until remaining substring matches
                    loop {
                        if match_substrings(&substring, &remaining_patterns, false, end_anchor) {
                            return true;
                        }
                        if !match_pattern(&mut substring.chars().peekable(), pattern) {
                            break;
                        }
                        substring = substring[1..].to_string();
                    }
                    continue 'input_line;
                }
                Pattern::ZeroOrOne(pattern) => {
                    let iter_copy = substring_iter.clone();
                    if !match_pattern(&mut substring_iter, pattern) {
                        substring_iter = iter_copy;
                    }
                }
                Pattern::Alternation(alternations) => {
                    for patterns in alternations {
                        let substring = String::from_iter(substring_iter.clone());
                        let remaining_patterns = pattern_iter.clone();
                        let patterns = patterns
                            .iter()
                            .chain(remaining_patterns)
                            .cloned()
                            .collect::<Vec<_>>();
                        if match_substrings(&substring, &patterns, false, end_anchor) {
                            return true;
                        }
                    }
                    continue 'input_line;
                }
            }
        }
        if end_anchor && substring_iter.next().is_some() {
            continue;
        }
        return true;
    }
    false
}

fn match_pattern(input_iter: &mut Peekable<Chars>, pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Literal(l) => match_literal(input_iter, *l),
        Pattern::Digit => match_digit(input_iter),
        Pattern::Alphanumeric => match_alphanumeric(input_iter),
        Pattern::Group(positive, group) => match_group(input_iter, group) == *positive,
        Pattern::Wildcard => match_wildcard(input_iter),
        p => panic!("Unexpected pattern: {:?}", p),
    }
}

fn match_literal(chars: &mut Peekable<Chars>, literal: char) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c == literal)
}
fn match_digit(chars: &mut Peekable<Chars>) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c.is_ascii_digit())
}
fn match_alphanumeric(chars: &mut Peekable<Chars>) -> bool {
    let c = chars.next();
    c.is_some_and(|c| c.is_alphanumeric())
}
fn match_group(chars: &mut Peekable<Chars>, group: &str) -> bool {
    let c = chars.next();
    c.is_some_and(|c| group.contains(c))
}
fn match_wildcard(chars: &mut Peekable<Chars>) -> bool {
    let c = chars.next();
    c.is_some()
}
