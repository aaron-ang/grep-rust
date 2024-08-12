use std::{iter::Peekable, str::Chars};

use crate::pattern::{Count, Pattern};

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
        Pattern::Alphanumeric(count) => {
            match_count(input_line, *count, |c| c.is_alphanumeric(), current_group)
        }
        Pattern::Wildcard(count) => {
            let restricted_chars = "\\[](|)";
            match_count(
                input_line,
                *count,
                |c| !restricted_chars.contains(*c),
                current_group,
            )
        }
        Pattern::CharGroup(positive, group, count) => match_count(
            input_line,
            *count,
            |c| group.contains(*c) ^ !positive,
            current_group,
        ),
        Pattern::Alternation(alternations) => {
            let mut current_group = String::new();
            for alt in alternations {
                let mut input_clone = input_line.clone();
                if alt.iter().all(|pattern| {
                    match_substring(
                        &mut input_clone,
                        pattern,
                        captured_groups,
                        &mut current_group,
                    )
                }) {
                    captured_groups.push(current_group);
                    *input_line = input_clone;
                    return true;
                }
                current_group.clear();
            }
            false
        }
        Pattern::CapturedGroup(group) => {
            let mut current_group = String::new();
            if group.iter().all(|pattern| {
                match_substring(input_line, pattern, captured_groups, &mut current_group)
            }) {
                captured_groups.push(current_group);
                true
            } else {
                false
            }
        }
        Pattern::Backreference(n) => captured_groups.get(*n as usize - 1).is_some_and(|matched| {
            let chars: String = input_line.take(matched.len()).collect();
            matched == &chars
        }),
    }
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
