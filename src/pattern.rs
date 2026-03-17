use std::{iter::Peekable, str::Chars};

use crate::parser::Parser;

#[derive(Debug, Clone)]
pub enum Pattern {
    /// A literal character, with a quantifier
    Literal(char, Count),
    /// A digit class [0-9], with a quantifier
    Digit(Count),
    /// An alphanumeric class [A-Za-z0-9_] (underscore allowed), with a quantifier
    Alphanumeric(Count),
    Wildcard(Count),
    /// A simple character group (possibly negated), with a quantifier
    CharGroup(bool, String, Count),
    Alternation(Alternation),
    /// Capturing group: ( ... ) with group index and quantifier
    CapturedGroup(Group),
    /// Backreference: \1, \2, ...
    Backreference(usize),
}

impl Pattern {
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

    fn match_substring(
        &mut self,
        input_line: &mut Peekable<Chars>,
        captured_groups: &mut Vec<String>,
        current_group: &mut String,
    ) -> bool {
        match self {
            Pattern::Literal(l, count) => count.match_count(input_line, |c| c == l, current_group),
            Pattern::Digit(count) => {
                count.match_count(input_line, |c| c.is_ascii_digit(), current_group)
            }
            Pattern::Alphanumeric(count) => count.match_count(
                input_line,
                |c| c.is_ascii_alphanumeric() || *c == '_',
                current_group,
            ),
            Pattern::Wildcard(count) => {
                let restricted_chars = "\\[](|)";
                count.match_count(
                    input_line,
                    |c| !restricted_chars.contains(*c),
                    current_group,
                )
            }
            Pattern::CharGroup(negated, group, count) => count.match_count(
                input_line,
                |c| c.is_ascii_alphanumeric() && (group.contains(*c) ^ *negated),
                current_group,
            ),
            Pattern::Alternation(alternation) => {
                alternation.match_with_count(input_line, captured_groups, current_group)
            }
            Pattern::CapturedGroup(group) => {
                group.match_with_count(input_line, captured_groups, current_group)
            }
            Pattern::Backreference(n) => {
                match_backreference(n, input_line, captured_groups, current_group)
            }
        }
    }

    fn has_greedy_quantifier(&self) -> bool {
        match self {
            Pattern::Literal(_, count) => count.is_greedy(),
            Pattern::Digit(count) => count.is_greedy(),
            Pattern::Alphanumeric(count) => count.is_greedy(),
            Pattern::Wildcard(count) => count.is_greedy(),
            Pattern::CharGroup(_, _, count) => count.is_greedy(),
            Pattern::Alternation(alt) => alt.count.is_greedy(),
            Pattern::CapturedGroup(group) => group.count.is_greedy(),
            Pattern::Backreference(_) => false,
        }
    }

    fn is_bounded_range(&self) -> bool {
        matches!(
            self,
            Pattern::Literal(_, Count::Range(_, _))
                | Pattern::Digit(Count::Range(_, _))
                | Pattern::Alphanumeric(Count::Range(_, _))
                | Pattern::Wildcard(Count::Range(_, _))
                | Pattern::CharGroup(_, _, Count::Range(_, _))
                | Pattern::Alternation(Alternation {
                    count: Count::Range(_, _),
                    ..
                })
                | Pattern::CapturedGroup(Group {
                    count: Count::Range(_, _),
                    ..
                })
        )
    }

    fn repetition_bounds(&self) -> Option<(usize, Option<usize>)> {
        let count = match self {
            Pattern::Literal(_, count)
            | Pattern::Digit(count)
            | Pattern::Alphanumeric(count)
            | Pattern::Wildcard(count)
            | Pattern::CharGroup(_, _, count) => *count,
            Pattern::Alternation(alternation) => alternation.count,
            Pattern::CapturedGroup(group) => group.count,
            Pattern::Backreference(_) => return None,
        };

        match count {
            Count::One => None,
            Count::OneOrMore => Some((1, None)),
            Count::ZeroOrOne => Some((0, Some(1))),
            Count::ZeroOrMore => Some((0, None)),
            Count::Exact(_) => None,
            Count::AtLeast(min) => Some((min, None)),
            Count::Range(min, max) => Some((min, Some(max))),
        }
    }

    fn as_single_match(&self) -> Pattern {
        match self {
            Pattern::Literal(ch, _) => Pattern::Literal(*ch, Count::One),
            Pattern::Digit(_) => Pattern::Digit(Count::One),
            Pattern::Alphanumeric(_) => Pattern::Alphanumeric(Count::One),
            Pattern::Wildcard(_) => Pattern::Wildcard(Count::One),
            Pattern::CharGroup(negated, group, _) => {
                Pattern::CharGroup(*negated, group.clone(), Count::One)
            }
            Pattern::Alternation(alternation) => Pattern::Alternation(Alternation {
                idx: alternation.idx,
                alternatives: alternation.alternatives.clone(),
                count: Count::One,
            }),
            Pattern::CapturedGroup(group) => Pattern::CapturedGroup(Group {
                idx: group.idx,
                patterns: group.patterns.clone(),
                count: Count::One,
            }),
            Pattern::Backreference(n) => Pattern::Backreference(*n),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Count {
    One,
    OneOrMore,
    ZeroOrOne,
    ZeroOrMore,
    Exact(usize),
    AtLeast(usize),
    Range(usize, usize),
}

impl Count {
    fn is_greedy(self) -> bool {
        matches!(
            self,
            Count::OneOrMore | Count::ZeroOrMore | Count::AtLeast(_) | Count::Range(_, _)
        )
    }

    fn match_count(
        self,
        input_line: &mut Peekable<Chars>,
        pred: impl Fn(&char) -> bool,
        current_group: &mut String,
    ) -> bool {
        match self {
            Self::One => input_line
                .next_if(&pred)
                .inspect(|c| current_group.push(*c))
                .is_some(),
            Self::OneOrMore => {
                let mut matched = false;
                while let Some(c) = input_line.next_if(&pred) {
                    current_group.push(c);
                    matched = true;
                }
                matched
            }
            Self::ZeroOrOne => {
                if let Some(c) = input_line.next_if(&pred) {
                    current_group.push(c);
                }
                true
            }
            Self::ZeroOrMore => {
                while let Some(c) = input_line.next_if(&pred) {
                    current_group.push(c);
                }
                true
            }
            Self::Exact(n) => {
                for _ in 0..n {
                    let Some(c) = input_line.next_if(&pred) else {
                        return false;
                    };
                    current_group.push(c);
                }
                true
            }
            Self::AtLeast(n) => {
                for _ in 0..n {
                    let Some(c) = input_line.next_if(&pred) else {
                        return false;
                    };
                    current_group.push(c);
                }
                while let Some(c) = input_line.next_if(&pred) {
                    current_group.push(c);
                }
                true
            }
            Self::Range(min, max) => {
                for _ in 0..min {
                    let Some(c) = input_line.next_if(&pred) else {
                        return false;
                    };
                    current_group.push(c);
                }
                for _ in min..max {
                    let Some(c) = input_line.next_if(&pred) else {
                        break;
                    };
                    current_group.push(c);
                }
                true
            }
        }
    }

    fn match_repeated(self, mut match_once: impl FnMut() -> bool) -> bool {
        match self {
            Self::One => match_once(),
            Self::OneOrMore => {
                let mut matched = false;
                while match_once() {
                    matched = true;
                }
                matched
            }
            Self::ZeroOrOne => {
                match_once();
                true
            }
            Self::ZeroOrMore => {
                while match_once() {}
                true
            }
            Self::Exact(n) => {
                for _ in 0..n {
                    if !match_once() {
                        return false;
                    }
                }
                true
            }
            Self::AtLeast(n) => {
                for _ in 0..n {
                    if !match_once() {
                        return false;
                    }
                }
                while match_once() {}
                true
            }
            Self::Range(min, max) => {
                for _ in 0..min {
                    if !match_once() {
                        return false;
                    }
                }
                for _ in min..max {
                    if !match_once() {
                        break;
                    }
                }
                true
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Alternation {
    pub idx: usize,
    pub alternatives: Vec<Vec<Pattern>>,
    pub count: Count,
}

impl Alternation {
    fn match_once(
        &mut self,
        input_line: &mut Peekable<Chars>,
        captured_groups: &mut Vec<String>,
        current_group: &mut String,
    ) -> bool {
        let mut new_current_group = String::new();
        for alt in &mut self.alternatives {
            let mut input_clone = input_line.clone();
            if alt.iter_mut().all(|pattern| {
                pattern.match_substring(&mut input_clone, captured_groups, &mut new_current_group)
            }) {
                current_group.push_str(&new_current_group);
                if self.idx >= captured_groups.len() {
                    captured_groups.resize(self.idx + 1, String::new());
                }
                captured_groups[self.idx] = new_current_group;
                *input_line = input_clone;
                return true;
            }
            new_current_group.clear();
        }
        false
    }

    fn match_with_count(
        &mut self,
        input_line: &mut Peekable<Chars>,
        captured_groups: &mut Vec<String>,
        current_group: &mut String,
    ) -> bool {
        self.count
            .match_repeated(|| self.match_once(input_line, captured_groups, current_group))
    }
}

#[derive(Debug, Clone)]
pub struct Group {
    pub idx: usize,
    pub patterns: Vec<Pattern>,
    pub count: Count,
}

impl Group {
    fn match_with_count(
        &mut self,
        input_line: &mut Peekable<Chars>,
        captured_groups: &mut Vec<String>,
        current_group: &mut String,
    ) -> bool {
        self.count
            .match_repeated(|| self.match_once(input_line, captured_groups, current_group))
    }

    fn match_once(
        &mut self,
        input_line: &mut Peekable<Chars>,
        captured_groups: &mut Vec<String>,
        current_group: &mut String,
    ) -> bool {
        let mut new_current_group = String::new();
        if self.patterns.iter_mut().all(|pattern| {
            pattern.match_substring(input_line, captured_groups, &mut new_current_group)
        }) {
            current_group.push_str(&new_current_group);
            if self.idx >= captured_groups.len() {
                captured_groups.resize(self.idx + 1, String::new());
            }
            captured_groups[self.idx] = new_current_group;
            true
        } else {
            false
        }
    }
}

pub fn match_regex(input_line: &str, regex: &str) -> Option<String> {
    let input_str = input_line.trim();
    let mut input_line = input_str.chars().peekable();
    let (patterns, start, end) = Pattern::parse(regex);

    let mut groups = Vec::new();
    let mut current_group = String::new();

    loop {
        let mut input_start = input_line.clone();
        if match_patterns_with_backtracking(
            &mut input_start,
            &patterns,
            &mut groups,
            &mut current_group,
        ) {
            if end && input_start.peek().is_some() {
                return None;
            }
            return Some(current_group);
        }
        if start {
            return None;
        }
        input_line.next()?;
        current_group.clear();
        groups.clear();
    }
}

pub fn match_patterns_with_backtracking(
    input_line: &mut Peekable<Chars>,
    patterns: &[Pattern],
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    if patterns.is_empty() {
        return true;
    }

    let mut pattern = patterns[0].clone();
    let remaining_patterns = &patterns[1..];

    let mut input = input_line.clone();
    let mut temp_group = String::new();
    let mut temp_groups = captured_groups.clone();

    let matched_pattern = pattern.match_substring(&mut input, &mut temp_groups, &mut temp_group);

    if matched_pattern
        && match_patterns_with_backtracking(
            &mut input,
            remaining_patterns,
            &mut temp_groups,
            &mut temp_group,
        )
    {
        *input_line = input;
        current_group.push_str(&temp_group);
        *captured_groups = temp_groups;
        return true;
    }

    if matched_pattern && pattern.has_greedy_quantifier() {
        if pattern.is_bounded_range() {
            try_backtracking_range(
                input_line,
                &pattern,
                remaining_patterns,
                captured_groups,
                current_group,
            )
        } else {
            try_backtracking(
                input_line,
                &pattern,
                remaining_patterns,
                captured_groups,
                current_group,
            )
        }
    } else {
        false
    }
}

#[derive(Clone)]
struct BacktrackState<'a> {
    input: Peekable<Chars<'a>>,
    groups: Vec<String>,
    matched: String,
}

fn try_backtracking(
    input_line: &mut Peekable<Chars>,
    pattern: &Pattern,
    remaining_patterns: &[Pattern],
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    let Some((min, max)) = pattern.repetition_bounds() else {
        return false;
    };

    let states = collect_backtrack_states(input_line, pattern, captured_groups, max);
    let start = min.max(1);

    for state in states.iter().skip(start) {
        let mut input_clone = state.input.clone();
        let mut temp_group = String::new();
        let mut temp_groups = state.groups.clone();

        if match_patterns_with_backtracking(
            &mut input_clone,
            remaining_patterns,
            &mut temp_groups,
            &mut temp_group,
        ) {
            *input_line = input_clone;
            current_group.push_str(&temp_group);
            *captured_groups = temp_groups;
            return true;
        }
    }

    false
}

fn try_backtracking_range(
    input_line: &mut Peekable<Chars>,
    pattern: &Pattern,
    remaining_patterns: &[Pattern],
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    let Some((min, max)) = pattern.repetition_bounds() else {
        return false;
    };

    let states = collect_backtrack_states(input_line, pattern, captured_groups, max);

    for state in states[min..].iter().rev() {
        let mut input_clone = state.input.clone();
        let mut temp_group = state.matched.clone();
        let mut temp_groups = state.groups.clone();

        if match_patterns_with_backtracking(
            &mut input_clone,
            remaining_patterns,
            &mut temp_groups,
            &mut temp_group,
        ) {
            *input_line = input_clone;
            current_group.push_str(&temp_group);
            *captured_groups = temp_groups;
            return true;
        }
    }

    false
}

fn collect_backtrack_states<'a>(
    input_line: &Peekable<Chars<'a>>,
    pattern: &Pattern,
    captured_groups: &[String],
    max: Option<usize>,
) -> Vec<BacktrackState<'a>> {
    let single = pattern.as_single_match();
    let mut states = vec![BacktrackState {
        input: input_line.clone(),
        groups: captured_groups.to_vec(),
        matched: String::new(),
    }];

    let mut next_input = input_line.clone();
    let mut next_groups = captured_groups.to_vec();
    let mut next_matched = String::new();

    while max.is_none_or(|limit| states.len() - 1 < limit) {
        let mut piece = String::new();
        let mut single_pattern = single.clone();
        if !single_pattern.match_substring(&mut next_input, &mut next_groups, &mut piece) {
            break;
        }
        next_matched.push_str(&piece);
        states.push(BacktrackState {
            input: next_input.clone(),
            groups: next_groups.clone(),
            matched: next_matched.clone(),
        });
    }

    states
}

fn match_backreference(
    n: &usize,
    input_line: &mut Peekable<Chars>,
    captured_groups: &[String],
    current_group: &mut String,
) -> bool {
    captured_groups.get(*n - 1).is_some_and(|matched| {
        let chars: String = input_line.take(matched.len()).collect();
        if matched == &chars {
            current_group.push_str(&chars);
            return true;
        }
        false
    })
}
