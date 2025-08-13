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
            char::is_ascii_digit,
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Alphanumeric(count) => try_quantified(
            input,
            count,
            is_alnum_or_underscore,
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::Wildcard(count) => try_quantified(
            input,
            count,
            is_unrestricted_char,
            captured_groups,
            matched_acc,
            rest,
        ),
        Pattern::CharGroup(negated, group, count) => try_quantified(
            input,
            count,
            |c| is_char_in_group(c, *negated, group),
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
            |it, groups, acc| match_alternation(alternation, it, groups, acc, &[]),
        ),
        Pattern::CapturedGroup(group) => try_group_like(
            input,
            &group.count,
            captured_groups,
            matched_acc,
            rest,
            |it, groups, acc| match_captured_group(group, it, groups, acc, &[]),
        ),
        Pattern::Backreference(n) => {
            match_backreference(n, input, captured_groups, matched_acc)
                && match_sequence(input, rest, captured_groups, matched_acc)
        }
    }
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

/// Represents a candidate match attempt with its associated state at a point in the input
#[derive(Clone)]
struct MatchCandidate<'a> {
    input: Chars<'a>,
    matched: &'a str,
    groups: Vec<&'a str>,
}

/// Common logic for trying different repetition counts (quantifier handling)
fn try_repetitions<'a, F>(
    input: &mut Chars<'a>,
    count: &Count,
    max: usize,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
    mut generate_candidate: F,
) -> bool
where
    F: FnMut(&Chars<'a>, usize) -> Option<MatchCandidate<'a>>,
{
    // Determine repetition counts to try, in greedy order
    let counts = match count {
        Count::One => (max > 0).then_some(1).into_iter().collect::<Vec<_>>(),
        Count::OneOrMore => (1..=max).rev().collect::<Vec<_>>(),
        Count::ZeroOrOne => (0..=max.min(1)).rev().collect::<Vec<_>>(),
    };

    // Try each repetition count and recurse to the rest of the sequence
    for n in counts {
        let Some(candidate) = generate_candidate(input, n) else {
            continue;
        };

        let mut after_input = candidate.input;
        let mut after_acc = matched_acc.clone();
        after_acc.push_str(candidate.matched);
        let mut after_groups = candidate.groups;

        if match_sequence(&mut after_input, rest, &mut after_groups, &mut after_acc) {
            *input = after_input;
            *matched_acc = after_acc;
            *captured_groups = after_groups;
            return true;
        }
    }
    false
}

/// Quantified primitives (characters, classes, wildcard)
fn try_quantified<'a>(
    input: &mut Chars<'a>,
    count: &Count,
    pred: impl Fn(&char) -> bool,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    // Greedily determine the maximum times the predicate matches from the current position
    let max = input.clone().take_while(|c| pred(c)).count();
    let groups_clone = captured_groups.clone();
    try_repetitions(
        input,
        count,
        max,
        captured_groups,
        matched_acc,
        rest,
        |base_input, n| {
            let mut local_input = base_input.clone();
            let before = local_input.as_str();
            local_input.by_ref().take(n).all(|c| pred(&c)).then(|| {
                let after = local_input.as_str();
                let matched = &before[..before.len() - after.len()];
                MatchCandidate {
                    input: local_input,
                    matched,
                    groups: groups_clone.clone(),
                }
            })
        },
    )
}

/// Quantified group-like constructs (capturing groups, alternations)
fn try_group_like<'a>(
    input: &mut Chars<'a>,
    count: &Count,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
    mut run_once: impl FnMut(&mut Chars<'a>, &mut Vec<&'a str>, &mut String) -> bool,
) -> bool {
    // Pre-compute consecutive successful runs of the inner matcher.
    // Each run records the input position after the run, the substring it matched,
    // and a snapshot of captured groups after that run.
    let mut runs: Vec<MatchCandidate> = Vec::new();
    let mut local_input = input.clone();
    let start_before = local_input.as_str();

    loop {
        let before = local_input.as_str();
        let mut acc = String::new();
        let mut groups_snapshot = captured_groups.clone();
        if run_once(&mut local_input, &mut groups_snapshot, &mut acc) {
            // Prevent infinite loops on zero-length matches
            let after = local_input.as_str();
            if before.len() == after.len() {
                break;
            }
            let matched_slice = &before[..before.len() - after.len()];
            runs.push(MatchCandidate {
                input: local_input.clone(),
                matched: matched_slice,
                groups: groups_snapshot,
            });
        } else {
            break;
        }
    }

    let max_runs = runs.len();
    let groups_clone = captured_groups.clone();

    try_repetitions(
        input,
        count,
        max_runs,
        captured_groups,
        matched_acc,
        rest,
        |base_input, n| {
            if n == 0 {
                return Some(MatchCandidate {
                    input: base_input.clone(),
                    matched: "",
                    groups: groups_clone.clone(),
                });
            }

            if n > max_runs {
                return None;
            }

            // Input after n runs is the input of the nth run
            let after_input = runs[n - 1].input.clone();
            let after_str = after_input.as_str();
            // Combined matched slice is from the very start of runs to after n runs
            let matched_slice = &start_before[..start_before.len() - after_str.len()];
            // Captured groups are whatever snapshot resulted after the nth run
            let after_groups = runs[n - 1].groups.clone();

            Some(MatchCandidate {
                input: after_input,
                matched: matched_slice,
                groups: after_groups,
            })
        },
    )
}

/// Alternation node: index for capturing and list of alternative pattern sequences
#[derive(Debug, Clone)]
pub struct Alternation {
    pub idx: usize,
    pub alternatives: Vec<Vec<Pattern>>,
    pub count: Count,
}

/// Attempts to match one of the alternatives, recording the capture for the alternation group
fn match_alternation<'a>(
    alternation: &Alternation,
    input: &mut Chars<'a>,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    let idx = alternation.idx.saturating_sub(1);
    for alt in &alternation.alternatives {
        let before = input.as_str();
        let mut local_input = input.clone();
        let mut sink = String::new();

        if match_sequence(&mut local_input, alt, captured_groups, &mut sink) {
            // Ensure captured_groups is large enough
            if captured_groups.len() <= idx {
                captured_groups.resize(idx + 1, "");
            }
            let after = local_input.as_str();
            let alt_slice = &before[..before.len() - after.len()];
            captured_groups[idx] = alt_slice;

            let mut after_input = local_input;
            let mut after_acc = matched_acc.clone();
            after_acc.push_str(alt_slice);

            if match_sequence(&mut after_input, rest, captured_groups, &mut after_acc) {
                *input = after_input;
                *matched_acc = after_acc;
                return true;
            }
        }
    }
    false
}

/// Group node: index, inner pattern sequence, and quantifier
#[derive(Debug, Clone)]
pub struct Group {
    pub idx: usize,
    pub patterns: Vec<Pattern>,
    pub count: Count,
}

/// Matches a capturing group exactly once, updating the captured groups
fn match_captured_group<'a>(
    group: &Group,
    input: &mut Chars<'a>,
    captured_groups: &mut Vec<&'a str>,
    matched_acc: &mut String,
    rest: &[Pattern],
) -> bool {
    let before = input.as_str();
    let mut local_input = input.clone();
    let mut sink = String::new();

    if !match_sequence(
        &mut local_input,
        &group.patterns,
        captured_groups,
        &mut sink,
    ) {
        return false;
    }

    let idx = group.idx.saturating_sub(1);
    if captured_groups.len() <= idx {
        captured_groups.resize(idx + 1, "");
    }
    let after = local_input.as_str();
    let group_slice = &before[..before.len() - after.len()];
    captured_groups[idx] = group_slice;

    let mut after_input = local_input;
    let mut after_acc = matched_acc.clone();
    after_acc.push_str(group_slice);

    if match_sequence(&mut after_input, rest, captured_groups, &mut after_acc) {
        *input = after_input;
        *matched_acc = after_acc;
        return true;
    }

    false
}

/// Matches a backreference by requiring the exact previously-captured text
fn match_backreference<'a>(
    n: &usize,
    input: &mut Chars<'a>,
    captured_groups: &[&'a str],
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
