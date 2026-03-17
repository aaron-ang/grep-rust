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
            Pattern::Literal(_, count) => matches!(count, Count::OneOrMore | Count::ZeroOrMore),
            Pattern::Digit(count) => matches!(count, Count::OneOrMore | Count::ZeroOrMore),
            Pattern::Alphanumeric(count) => matches!(count, Count::OneOrMore | Count::ZeroOrMore),
            Pattern::Wildcard(count) => matches!(count, Count::OneOrMore | Count::ZeroOrMore),
            Pattern::CharGroup(_, _, count) => {
                matches!(count, Count::OneOrMore | Count::ZeroOrMore)
            }
            Pattern::Alternation(alt) => matches!(alt.count, Count::OneOrMore | Count::ZeroOrMore),
            Pattern::CapturedGroup(group) => {
                matches!(group.count, Count::OneOrMore | Count::ZeroOrMore)
            }
            Pattern::Backreference(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Count {
    One,
    OneOrMore,
    ZeroOrOne,
    ZeroOrMore,
}

impl Count {
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
        match self.count {
            Count::One => self.match_once(input_line, captured_groups, current_group),
            Count::OneOrMore => {
                let mut matched = false;
                while self.match_once(input_line, captured_groups, current_group) {
                    matched = true;
                }
                matched
            }
            Count::ZeroOrOne => {
                self.match_once(input_line, captured_groups, current_group);
                true
            }
            Count::ZeroOrMore => {
                while self.match_once(input_line, captured_groups, current_group) {}
                true
            }
        }
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
        match self.count {
            Count::One => self.match_once(input_line, captured_groups, current_group),
            Count::OneOrMore => {
                let mut matched = false;
                while self.match_once(input_line, captured_groups, current_group) {
                    matched = true;
                }
                matched
            }
            Count::ZeroOrOne => {
                self.match_once(input_line, captured_groups, current_group);
                true
            }
            Count::ZeroOrMore => {
                while self.match_once(input_line, captured_groups, current_group) {}
                true
            }
        }
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

    if pattern.match_substring(&mut input, &mut temp_groups, &mut temp_group)
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

    if pattern.has_greedy_quantifier() {
        try_backtracking(
            input_line,
            &pattern,
            remaining_patterns,
            captured_groups,
            current_group,
        )
    } else {
        false
    }
}

fn try_backtracking(
    input_line: &mut Peekable<Chars>,
    pattern: &Pattern,
    remaining_patterns: &[Pattern],
    captured_groups: &mut Vec<String>,
    current_group: &mut String,
) -> bool {
    let original_input = input_line.clone();
    let mut temp_input = input_line.clone();

    match pattern {
        Pattern::CharGroup(negated, group, Count::ZeroOrMore | Count::OneOrMore) => {
            while let Some(c) = temp_input.peek() {
                let matches = c.is_ascii_alphanumeric() && (group.contains(*c) ^ *negated);
                if !matches {
                    break;
                }
                temp_input.next();
                let mut input_clone = temp_input.clone();
                let mut temp_group = String::new();
                let mut temp_groups = captured_groups.clone();

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
        }
        Pattern::Digit(Count::ZeroOrMore | Count::OneOrMore) => {
            while let Some(c) = temp_input.peek() {
                if !c.is_ascii_digit() {
                    break;
                }
                temp_input.next();
                let mut input_clone = temp_input.clone();
                let mut temp_group = String::new();
                let mut temp_groups = captured_groups.clone();

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
        }
        _ => {
            while let Some(_c) = temp_input.next() {
                let mut input_clone = temp_input.clone();
                let mut temp_group = String::new();
                let mut temp_groups = captured_groups.clone();

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
        }
    }

    *input_line = original_input;
    false
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
