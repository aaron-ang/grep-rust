use std::{iter::Peekable, str::Chars};

use crate::pattern::{Alternation, Count, Group, Pattern};

pub fn match_substring(
    input_line: &mut Peekable<Chars>,
    pattern: &Pattern,
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    match pattern {
        Pattern::Literal(l, count) => match_count(input_line, *count, |c| c == l, current_group),
        Pattern::Digit(count) => {
            match_count(input_line, *count, |c| c.is_ascii_digit(), current_group)
        }
        Pattern::Alphanumeric(count) => match_count(
            input_line,
            *count,
            |c| c.is_ascii_alphanumeric(),
            current_group,
        ),
        Pattern::Wildcard(count) => {
            let restricted_chars = "\\[](|)";
            match_count(
                input_line,
                *count,
                |c| !restricted_chars.contains(*c),
                current_group,
            )
        }
        Pattern::CharGroup(negated, group, count) => match_count(
            input_line,
            *count,
            |c| c.is_ascii_alphanumeric() && (group.contains(*c) ^ negated),
            current_group,
        ),
        Pattern::Alternation(alternation) => {
            match_alternation(alternation, input_line, captured_groups, current_group)
        }
        Pattern::CapturedGroup(group) => {
            match_captured_group(group, input_line, captured_groups, current_group)
        }
        Pattern::Backreference(n) => {
            match_backreference(n, input_line, captured_groups, current_group)
        }
    }
}

fn match_alternation(
    alternation: &Alternation,
    input_line: &mut Peekable<Chars>,
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    let mut new_current_group = String::new();
    for alt in &alternation.alternatives {
        let mut input_clone = input_line.clone();
        if alt.iter().all(|pattern| {
            match_substring(
                &mut input_clone,
                pattern,
                captured_groups,
                &mut new_current_group,
            )
        }) {
            current_group.push_str(&new_current_group);
            let i = alternation.idx - 1;
            if i >= captured_groups.len() {
                captured_groups.resize(i + 1, String::new());
            }
            captured_groups[i] = new_current_group;
            *input_line = input_clone;
            return true;
        }
        new_current_group.clear();
    }
    false
}

fn match_captured_group(
    group: &Group,
    input_line: &mut Peekable<Chars>,
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    let mut new_current_group = String::new();
    if group.patterns.iter().all(|pattern| {
        match_substring(input_line, pattern, captured_groups, &mut new_current_group)
    }) {
        current_group.push_str(&new_current_group);
        let i = group.idx - 1;
        if i >= captured_groups.len() {
            captured_groups.resize(i + 1, String::new());
        }
        captured_groups[i] = new_current_group;
        true
    } else {
        false
    }
}

fn match_backreference(
    n: &usize,
    input_line: &mut Peekable<Chars>,
    captured_groups: &Vec<String>,
    current_group: &mut String,
) -> bool {
    captured_groups.get(*n as usize - 1).is_some_and(|matched| {
        let chars: String = input_line.take(matched.len()).collect();
        if matched == &chars {
            current_group.push_str(&chars);
            return true;
        }
        false
    })
}

fn match_count(
    input_line: &mut Peekable<Chars>,
    count: Count,
    pred: impl Fn(&char) -> bool,
    current_group: &mut String,
) -> bool {
    match count {
        Count::One => input_line
            .next_if(&pred)
            .inspect(|c| current_group.push(*c))
            .is_some(),
        Count::OneOrMore => {
            let mut k = 0;
            while let Some(c) = input_line.next_if(&pred) {
                current_group.push(c);
                k += 1;
            }
            k >= 1
        }
        Count::ZeroOrOne => {
            if let Some(c) = input_line.next_if(&pred) {
                current_group.push(c);
            }
            true
        }
    }
}
