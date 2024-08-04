use std::str::Chars;

use crate::pattern::Pattern;

pub fn match_substrings(
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
        'pattern: while let Some(pattern) = pattern_iter.next() {
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
                    let mut substring = String::from_iter(substring_iter);
                    if !match_pattern(&substring, pattern) {
                        continue 'input_line; // zero match: go to next input substring
                    }
                    let remaining_patterns = pattern_iter.cloned().collect::<Vec<_>>();
                    // incrementally match `pattern` until remaining substring matches
                    loop {
                        substring = substring[1..].to_string();
                        if match_substrings(&substring, &remaining_patterns, false, false) {
                            return true;
                        }
                        if !match_pattern(&substring, pattern) {
                            break;
                        }
                    }
                    continue 'input_line;
                }
                Pattern::ZeroOrMore(pattern) => {
                    let mut substring = String::from_iter(substring_iter.clone());
                    if !match_pattern(&substring, pattern) {
                        continue; // zero match: go to next pattern
                    }
                    let remaining_patterns = pattern_iter.cloned().collect::<Vec<_>>();
                    loop {
                        substring = substring[1..].to_string();
                        if match_substrings(&substring, &remaining_patterns, false, false) {
                            return true;
                        }
                        if !match_pattern(&substring, pattern) {
                            break;
                        }
                    }
                    continue 'input_line;
                }
                Pattern::Wildcard => {
                    if !match_wildcard(&mut substring_iter) {
                        continue 'input_line;
                    }
                }
                Pattern::Alternation(alternations) => {
                    'alt: for alt in alternations {
                        let mut iter_copy = substring_iter.clone();
                        for pattern in alt {
                            let substring = String::from_iter(iter_copy.clone());
                            if !match_pattern(&substring, pattern) {
                                continue 'alt;
                            }
                            iter_copy.next();
                        }
                        substring_iter = iter_copy;
                        continue 'pattern;
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

fn match_wildcard(chars: &mut Chars) -> bool {
    let c = chars.next();
    c.is_some()
}
