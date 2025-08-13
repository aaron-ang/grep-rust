use std::str::Chars;

use crate::parser::Parser;

/// Entry point: attempts to match `regex` against `input_line` and returns the matched substring
pub fn match_regex(input_line: &str, regex: &str) -> Option<String> {
    let trimmed_input = input_line.trim();
    let (patterns, start_anchor, end_anchor) = parse(regex);

    let try_match = |input: &str| {
        let mut input_chars = input.chars();
        let mut groups = Vec::new();
        let mut matched = String::new();
        if match_sequence(&mut input_chars, &patterns, &mut groups, &mut matched)
            && (!end_anchor || input_chars.as_str().is_empty())
        {
            Some(matched)
        } else {
            None
        }
    };

    if start_anchor {
        try_match(trimmed_input)
    } else {
        trimmed_input
            .char_indices()
            .find_map(|(i, _)| try_match(&trimmed_input[i..]))
    }
}

/// Quantifiers supported by this engine
#[derive(Debug, Clone, Copy)]
pub enum Count {
    /// Exactly one occurrence
    One,
    /// One or more (greedy)
    OneOrMore,
    /// Zero or one (greedy)
    ZeroOrOne,
}

/// AST produced by the parser for the simplified regex grammar
#[derive(Debug, Clone)]
pub enum Pattern {
    /// A literal character, with a quantifier
    Literal(char, Count),
    /// A digit class [0-9], with a quantifier
    Digit(Count),
    /// An alphanumeric class [A-Za-z0-9_] (underscore allowed), with a quantifier
    Alphanumeric(Count),
    /// A wildcard that excludes regex metacharacters used in this implementation
    Wildcard(Count),
    /// A simple character group (possibly negated), with a quantifier
    CharGroup(bool, String, Count),
    /// Alternation: (A|B|C) with group index and quantifier
    Alternation(Alternation),
    /// Capturing group: ( ... ) with group index and quantifier
    CapturedGroup(Group),
    /// Backreference: \1, \2, ...
    Backreference(usize),
}

/// Alternation node: index for capturing and list of alternative pattern sequences
#[derive(Debug, Clone)]
pub struct Alternation {
    pub idx: usize,
    pub alternatives: Vec<Vec<Pattern>>,
    pub count: Count,
}

/// Group node: index, inner pattern sequence, and quantifier
#[derive(Debug, Clone)]
pub struct Group {
    pub idx: usize,
    pub patterns: Vec<Pattern>,
    pub count: Count,
}

/// Parses the regex into a sequence of `Pattern`s and extracts anchors
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

/// Recursive matcher that tries to match a sequence of patterns against the input
fn match_sequence<'a>(
    input: &mut Chars<'a>,
    patterns: &[Pattern],
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
) -> bool {
    if patterns.is_empty() {
        return true;
    }

    let (first, rest) = patterns.split_first().unwrap();
    match first {
        Pattern::Literal(ch, count) => {
            match_repeated_pred(input, count, captured_groups, matched_acc, rest, |c| {
                c == ch
            })
        }
        Pattern::Digit(count) => match_repeated_pred(
            input,
            count,
            captured_groups,
            matched_acc,
            rest,
            char::is_ascii_digit,
        ),
        Pattern::Alphanumeric(count) => match_repeated_pred(
            input,
            count,
            captured_groups,
            matched_acc,
            rest,
            is_alnum_or_underscore,
        ),
        Pattern::Wildcard(count) => match_repeated_pred(
            input,
            count,
            captured_groups,
            matched_acc,
            rest,
            is_unrestricted_char,
        ),
        Pattern::CharGroup(negated, group, count) => {
            match_repeated_pred(input, count, captured_groups, matched_acc, rest, |c| {
                is_char_in_group(c, *negated, group)
            })
        }
        Pattern::Alternation(alternation) => match_repeated(
            input,
            &alternation.count,
            captured_groups,
            matched_acc,
            rest,
            |it, groups| run_once_alternatives(it, groups, alternation),
        ),
        Pattern::CapturedGroup(group) => match_repeated(
            input,
            &group.count,
            captured_groups,
            matched_acc,
            rest,
            |it, groups| run_once_patterns(it, groups, &group.patterns, group.idx),
        ),
        Pattern::Backreference(n) => {
            match_backreference(n, input, captured_groups, matched_acc)
                && match_sequence(input, rest, captured_groups, matched_acc)
        }
    }
}

fn match_repeated_pred<'a, P>(
    input: &mut Chars<'a>,
    count: &Count,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
    mut pred: P,
) -> bool
where
    P: FnMut(&char) -> bool,
{
    match_repeated(
        input,
        count,
        captured_groups,
        matched_acc,
        rest,
        |it, _groups| {
            if let Some(c) = it.clone().next() {
                if pred(&c) {
                    it.next();
                    return true;
                }
            }
            false
        },
    )
}

/// Represents a candidate match attempt with its associated state at a point in the input
#[derive(Clone)]
struct MatchCandidate<'a> {
    input: Chars<'a>,
    matched: &'a str,
    groups: Vec<&'a str>,
}

fn match_repeated<'a>(
    input: &mut Chars<'a>,
    count: &Count,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
    mut run_once: impl FnMut(&mut Chars<'a>, &mut Vec<&'a str>) -> bool,
) -> bool {
    // Pre-compute consecutive successful runs of the inner matcher
    let mut runs = Vec::new();
    let mut local_input = input.clone();
    let start_before = local_input.as_str();

    loop {
        let before = local_input.as_str();
        let mut groups = captured_groups.clone();
        if !run_once(&mut local_input, &mut groups) {
            break;
        }

        let after = local_input.as_str();
        let matched = consumed_slice(before, after);
        runs.push(MatchCandidate {
            input: local_input.clone(),
            matched,
            groups,
        });
    }

    // Determine which repetition counts to try, in greedy order
    let max_runs = runs.len();
    let counts_to_try = match count {
        Count::One => {
            if max_runs > 0 {
                vec![1]
            } else {
                vec![]
            }
        }
        Count::OneOrMore => (1..=max_runs).rev().collect(),
        Count::ZeroOrOne => (0..=max_runs.min(1)).rev().collect(),
    };

    // Try each repetition count
    for n in counts_to_try {
        let candidate = if n == 0 {
            MatchCandidate {
                input: input.clone(),
                matched: "",
                groups: captured_groups.clone(),
            }
        } else {
            let run = &runs[n - 1];
            MatchCandidate {
                input: run.input.clone(),
                matched: consumed_slice(start_before, run.input.as_str()),
                groups: run.groups.clone(),
            }
        };

        let mut test_input = candidate.input;
        let mut test_acc = matched_acc.clone();
        test_acc.push_str(candidate.matched);
        let mut test_groups = candidate.groups;

        if match_sequence(&mut test_input, rest, &mut test_groups, &mut test_acc) {
            *input = test_input;
            *matched_acc = test_acc;
            *captured_groups = test_groups;
            return true;
        }
    }

    false
}

fn run_once_alternatives<'a>(
    input: &mut Chars<'a>,
    captured_groups: &mut Vec<&'a str>,
    alternation: &Alternation,
) -> bool {
    let input_start = input.as_str();

    for alternative in &alternation.alternatives {
        let mut candidate_input = input.clone();
        let mut candidate_groups = captured_groups.clone();
        let mut matched_text = String::new();

        if match_sequence(
            &mut candidate_input,
            alternative,
            &mut candidate_groups,
            &mut matched_text,
        ) {
            // Calculate the matched slice for this alternative
            let input_end = candidate_input.as_str();
            let matched_slice = consumed_slice(input_start, input_end);

            // Store the matched text in the capture group
            set_capture(&mut candidate_groups, alternation.idx, matched_slice);

            // Update the original state and return success
            *input = candidate_input;
            *captured_groups = candidate_groups;
            return true;
        }
    }

    false
}

fn run_once_patterns<'a>(
    input: &mut Chars<'a>,
    captured_groups: &mut Vec<&'a str>,
    patterns: &[Pattern],
    idx: usize,
) -> bool {
    let input_start = input.as_str();
    let mut candidate_input = input.clone();
    let mut candidate_groups = captured_groups.clone();
    let mut matched_text = String::new();

    if match_sequence(
        &mut candidate_input,
        patterns,
        &mut candidate_groups,
        &mut matched_text,
    ) {
        // Calculate the matched slice for this group
        let input_end = candidate_input.as_str();
        let matched_slice = consumed_slice(input_start, input_end);

        // Store the matched text in the capture group
        set_capture(&mut candidate_groups, idx, matched_slice);

        // Update the original state and return success
        *input = candidate_input;
        *captured_groups = candidate_groups;
        return true;
    }

    false
}

/// Matches a backreference by requiring the exact previously-captured text
fn match_backreference(
    n: &usize,
    input: &mut Chars,
    captured_groups: &[&str],
    matched_acc: &mut String,
) -> bool {
    let Some(expected) = captured_groups.get(n.saturating_sub(1)) else {
        return false;
    };
    if !input.as_str().starts_with(expected) {
        return false;
    }
    input.nth(expected.len().saturating_sub(1));
    matched_acc.push_str(expected);
    true
}

fn is_alnum_or_underscore(c: &char) -> bool {
    c.is_ascii_alphanumeric() || *c == '_'
}

fn is_unrestricted_char(c: &char) -> bool {
    let metachars = "\\[](|)";
    !metachars.contains(*c)
}

fn is_char_in_group(c: &char, negated: bool, group: &str) -> bool {
    c.is_ascii_alphanumeric() && (group.contains(*c) ^ negated)
}

fn consumed_slice<'a>(before: &'a str, after: &str) -> &'a str {
    &before[..before.len().saturating_sub(after.len())]
}

fn set_capture<'a>(captures: &mut Vec<&'a str>, idx: usize, value: &'a str) {
    ensure_group_capacity(captures, idx);
    captures[idx] = value;
}

fn ensure_group_capacity(captured_groups: &mut Vec<&str>, idx: usize) {
    if captured_groups.len() <= idx {
        captured_groups.resize(idx + 1, "");
    }
}
