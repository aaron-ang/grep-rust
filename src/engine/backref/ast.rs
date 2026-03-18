use crate::engine::RegexMatch;

use super::{parser::Parser, runtime::CompiledBackreferenceRegex};

#[derive(Debug)]
pub(super) enum Pattern {
    Literal(char, Count),
    Digit(Count),
    Alphanumeric(Count),
    Wildcard(Count),
    CharGroup(CharGroup, Count),
    Alternation {
        idx: usize,
        alternatives: Vec<Vec<Pattern>>,
        count: Count,
    },
    CapturedGroup {
        idx: usize,
        patterns: Vec<Pattern>,
        count: Count,
    },
    Backreference(usize),
}

impl Pattern {
    pub(super) fn parse(
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

    pub(super) fn count(&self) -> Count {
        match self {
            Pattern::Literal(_, count)
            | Pattern::Digit(count)
            | Pattern::Alphanumeric(count)
            | Pattern::Wildcard(count)
            | Pattern::CharGroup(_, count)
            | Pattern::Alternation { count, .. }
            | Pattern::CapturedGroup { count, .. } => *count,
            Pattern::Backreference(_) => Count::One,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Count {
    One,
    OneOrMore,
    ZeroOrOne,
    ZeroOrMore,
    Exact(usize),
    AtLeast(usize),
    Range(usize, usize),
}

impl Count {
    pub(super) fn is_exactly_one(self) -> bool {
        matches!(self, Count::One | Count::Exact(1))
    }

    pub(super) fn fixed_repetitions(self) -> Option<usize> {
        match self {
            Count::One => Some(1),
            Count::Exact(n) => Some(n),
            _ => None,
        }
    }

    pub(super) fn repetition_bounds(self) -> (usize, Option<usize>) {
        match self {
            Count::One => (1, Some(1)),
            Count::OneOrMore => (1, None),
            Count::ZeroOrOne => (0, Some(1)),
            Count::ZeroOrMore => (0, None),
            Count::Exact(n) => (n, Some(n)),
            Count::AtLeast(min) => (min, None),
            Count::Range(min, max) => (min, Some(max)),
        }
    }

    pub(super) fn combine(&self, other: &Self) -> Self {
        let (left_min, left_max) = self.repetition_bounds();
        let (right_min, right_max) = other.repetition_bounds();
        let min = left_min + right_min;
        let max = match (left_max, right_max) {
            (Some(left), Some(right)) => Some(left + right),
            _ => None,
        };

        match (min, max) {
            (1, Some(1)) => Count::One,
            (0, Some(1)) => Count::ZeroOrOne,
            (1, None) => Count::OneOrMore,
            (0, None) => Count::ZeroOrMore,
            (exact, Some(upper)) if exact == upper => Count::Exact(exact),
            (lower, None) => Count::AtLeast(lower),
            (lower, Some(upper)) => Count::Range(lower, upper),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CharGroup {
    negated: bool,
    ascii_members: [bool; 128],
}

impl CharGroup {
    pub(super) fn new(negated: bool, members: &str) -> Self {
        let mut ascii_members = [false; 128];
        for member in members.chars() {
            if member.is_ascii() {
                ascii_members[member as usize] = true;
            }
        }

        Self {
            negated,
            ascii_members,
        }
    }

    pub(super) fn matches(&self, c: char) -> bool {
        c.is_ascii_alphanumeric() && (self.ascii_members[c as usize] ^ self.negated)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CaptureSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn compile_backreference_regex(regex: &str) -> CompiledBackreferenceRegex {
    CompiledBackreferenceRegex::new(regex)
}

pub(super) fn find_all_backreference_regex_spans_compiled(
    input_line: &str,
    regex: &CompiledBackreferenceRegex,
) -> Vec<RegexMatch> {
    regex.find_all(input_line)
}
