use std::{iter::Peekable, str::Chars};

use crate::parser::Parser;

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(char, Count),
    Digit(Count),
    Alphanumeric(Count),
    Wildcard(Count),
    CharGroup(bool, String, Count),
    Alternation(Alternation),
    CapturedGroup(Group),
    Backreference(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum Count {
    One,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, Clone)]
pub struct Alternation {
    pub idx: usize,
    pub alternatives: Vec<Vec<Pattern>>,
    pub count: Count,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub idx: usize,
    pub patterns: Vec<Pattern>,
    pub count: Count,
}

pub fn match_regex(input_line: &str, regex: &str) -> Option<String> {
    let trimmed_input = input_line.trim();
    let (patterns, start_anchor, end_anchor) = parse(regex);

    let try_match = |input: &str| {
        let mut input_chars = input.chars().peekable();
        let mut groups = Vec::new();
        let mut matched = String::new();
        if match_sequence(&mut input_chars, &patterns, &mut groups, &mut matched)
            && (!end_anchor || input_chars.peek().is_none())
        {
            return Some(matched);
        }
        None
    };

    if start_anchor {
        try_match(trimmed_input)
    } else {
        (0..trimmed_input.len())
            .filter(|&i| trimmed_input.is_char_boundary(i))
            .find_map(|i| try_match(&trimmed_input[i..]))
    }
}

fn parse(
    regex: &str,
) -> (
    Vec<Pattern>,
    bool, /* start_anchor */
    bool, /* end_anchor */
) {
    let start = regex.starts_with('^');
    let end = regex.ends_with('$');
    let mut chars = regex.chars().peekable();

    if start {
        chars.next();
    }
    if end {
        chars.next_back();
    }

    let mut parser = Parser::new();
    let mut patterns = Vec::new();
    while chars.peek().is_some() {
        patterns.push(parser.parse(&mut chars));
    }

    (patterns, start, end)
}

fn match_sequence(
    input: &mut Peekable<Chars>,
    patterns: &[Pattern],
    captured_groups: &mut Vec<String>,
    matched_acc: &mut String,
) -> bool {
    if patterns.is_empty() {
        return true;
    }

    let (first, rest) = patterns.split_first().unwrap();
    match first {
        Pattern::Literal(ch, count) => try_quantified(
            input,
            count,
            |c| c == ch,
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Digit(count) => try_quantified(
            input,
            count,
            |c| c.is_ascii_digit(),
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Alphanumeric(count) => try_quantified(
            input,
            count,
            |c| c.is_ascii_alphanumeric() || *c == '_',
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Wildcard(count) => {
            let restricted_chars = "\\[](|)";
            try_quantified(
                input,
                count,
                move |c| !restricted_chars.contains(*c),
                captured_groups,
                matched_acc,
                rest,
            )
        }
        Pattern::CharGroup(negated, group, count) => try_quantified(
            input,
            count,
            |c| c.is_ascii_alphanumeric() && (group.contains(*c) ^ negated),
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Alternation(alternation) => try_group_like(
            input,
            &alternation.count,
            captured_groups,
            matched_acc,
            rest,
            |local_input, local_groups, local_acc| {
                match_alternation(alternation, local_input, local_groups, local_acc, &[])
            },
        ),
        Pattern::CapturedGroup(group) => try_group_like(
            input,
            &group.count,
            captured_groups,
            matched_acc,
            rest,
            |local_input, local_groups, local_acc| {
                match_captured_group(group, local_input, local_groups, local_acc, &[])
            },
        ),
        Pattern::Backreference(n) => {
            match_backreference(n, input, captured_groups, matched_acc)
                && match_sequence(input, rest, captured_groups, matched_acc)
        }
    }
}

fn try_quantified(
    input: &mut Peekable<Chars>,
    count: &Count,
    pred: impl Fn(&char) -> bool + Copy,
    captured_groups: &mut Vec<String>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    // Clone input and greedily consume as many matching chars as possible
    let mut greedy_input = input.clone();
    let mut greedy = String::new();
    while let Some(c) = greedy_input.next_if(&pred) {
        greedy.push(c);
    }
    let max = greedy.chars().count();

    // Determine repetition counts to try, in greedy order
    let tries: Vec<usize> = match count {
        Count::One if max >= 1 => vec![1],
        Count::OneOrMore if max >= 1 => (1..=max).rev().collect(),
        Count::ZeroOrOne if max >= 1 => vec![1, 0],
        Count::ZeroOrOne => vec![0],
        _ => vec![],
    };

    for n in tries {
        let mut local_input = input.clone();
        let mut consumed = String::new();
        for _ in 0..n {
            if let Some(c) = local_input.next_if(&pred) {
                consumed.push(c);
            }
        }
        if consumed.chars().count() != n {
            continue;
        }
        let mut after_acc = matched_acc.clone();
        after_acc.push_str(&consumed);
        let mut after_input = local_input;
        if match_sequence(&mut after_input, rest, captured_groups, &mut after_acc) {
            *input = after_input;
            *matched_acc = after_acc;
            return true;
        }
    }
    false
}

fn try_group_like(
    input: &mut Peekable<Chars>,
    count: &Count,
    captured_groups: &mut Vec<String>,
    matched_acc: &mut String,
    rest: &[Pattern],
    mut run_once: impl FnMut(&mut Peekable<Chars>, &mut Vec<String>, &mut String) -> bool,
) -> bool {
    // Collect all consecutive successful runs of the inner matcher
    let mut run_inputs = Vec::new();
    let mut run_matches = Vec::new();
    let mut local_input = input.clone();

    loop {
        let mut acc = String::new();
        let mut groups_snapshot = captured_groups.clone();
        if run_once(&mut local_input, &mut groups_snapshot, &mut acc) {
            // Prevent infinite loops on zero-length matches
            if acc.is_empty() {
                break;
            }
            run_inputs.push(local_input.clone());
            run_matches.push((acc, groups_snapshot));
        } else {
            break;
        }
    }

    let max_runs = run_matches.len();
    let candidates = match count {
        Count::One => {
            if max_runs >= 1 {
                vec![1]
            } else {
                vec![]
            }
        }
        Count::OneOrMore => (1..=max_runs).rev().collect(),
        Count::ZeroOrOne => {
            if max_runs >= 1 {
                vec![1, 0]
            } else {
                vec![0]
            }
        }
    };

    for k in candidates {
        let mut after_input = if k == 0 {
            input.clone()
        } else {
            run_inputs[k - 1].clone()
        };
        let mut after_acc = matched_acc.clone();
        let mut after_groups = captured_groups.clone();

        for run_match in run_matches.iter().take(k) {
            after_acc.push_str(&run_match.0);
            after_groups = run_match.1.clone();
        }

        if match_sequence(&mut after_input, rest, &mut after_groups, &mut after_acc) {
            *input = after_input;
            *matched_acc = after_acc;
            *captured_groups = after_groups;
            return true;
        }
    }
    false
}

fn match_alternation(
    alternation: &Alternation,
    input: &mut Peekable<Chars>,
    captured_groups: &mut Vec<String>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    let idx = alternation.idx.saturating_sub(1);
    for alt in &alternation.alternatives {
        let mut local_input = input.clone();
        let mut local_group_acc = String::new();

        if match_sequence(&mut local_input, alt, captured_groups, &mut local_group_acc) {
            // Ensure captured_groups is large enough
            if captured_groups.len() <= idx {
                captured_groups.resize(idx + 1, String::new());
            }
            captured_groups[idx] = local_group_acc.clone();

            let mut after_input = local_input;
            let mut after_acc = matched_acc.clone();
            after_acc.push_str(&local_group_acc);

            if match_sequence(&mut after_input, rest, captured_groups, &mut after_acc) {
                *input = after_input;
                *matched_acc = after_acc;
                return true;
            }
        }
    }
    false
}

fn match_captured_group(
    group: &Group,
    input: &mut Peekable<Chars>,
    captured_groups: &mut Vec<String>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    let mut local_input = input.clone();
    let mut group_acc = String::new();

    if !match_sequence(
        &mut local_input,
        &group.patterns,
        captured_groups,
        &mut group_acc,
    ) {
        return false;
    }

    let idx = group.idx.saturating_sub(1);
    if captured_groups.len() <= idx {
        captured_groups.resize(idx + 1, String::new());
    }
    captured_groups[idx] = group_acc.clone();

    let mut after_input = local_input;
    let mut after_acc = matched_acc.clone();
    after_acc.push_str(&group_acc);

    if match_sequence(&mut after_input, rest, captured_groups, &mut after_acc) {
        *input = after_input;
        *matched_acc = after_acc;
        return true;
    }

    false
}

fn match_backreference(
    n: &usize,
    input: &mut Peekable<Chars>,
    captured_groups: &[String],
    matched_acc: &mut String,
) -> bool {
    let idx = n.saturating_sub(1);
    if let Some(expected) = captured_groups.get(idx) {
        let mut local_input = input.clone();
        let mut consumed = String::new();
        for ch in expected.chars() {
            if let Some(c) = local_input.next_if(|x| *x == ch) {
                consumed.push(c);
            } else {
                return false;
            }
        }
        *input = local_input;
        matched_acc.push_str(&consumed);
        return true;
    }
    false
}
