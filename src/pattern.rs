use crate::{engine::RegexMatch, parser::Parser};

#[derive(Debug, Clone)]
pub enum Pattern {
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

    fn count(&self) -> Count {
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

    fn modifies_captures(&self) -> bool {
        matches!(
            self,
            Pattern::Alternation { .. } | Pattern::CapturedGroup { .. }
        )
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
    fn is_exactly_one(self) -> bool {
        matches!(self, Count::One | Count::Exact(1))
    }

    fn is_zero_or_one(self) -> bool {
        matches!(self, Count::ZeroOrOne)
    }

    fn fixed_repetitions(self) -> Option<usize> {
        match self {
            Count::One => Some(1),
            Count::Exact(n) => Some(n),
            _ => None,
        }
    }

    fn repetition_bounds(self) -> (usize, Option<usize>) {
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

    fn from_bounds(min: usize, max: Option<usize>) -> Self {
        match (min, max) {
            (1, Some(1)) => Count::One,
            (0, Some(1)) => Count::ZeroOrOne,
            (1, None) => Count::OneOrMore,
            (0, None) => Count::ZeroOrMore,
            (exact, Some(max)) if exact == max => Count::Exact(exact),
            (min, None) => Count::AtLeast(min),
            (min, Some(max)) => Count::Range(min, max),
        }
    }

    fn combine(self, other: Self) -> Self {
        let (left_min, left_max) = self.repetition_bounds();
        let (right_min, right_max) = other.repetition_bounds();
        let min = left_min + right_min;
        let max = match (left_max, right_max) {
            (Some(left), Some(right)) => Some(left + right),
            _ => None,
        };

        Count::from_bounds(min, max)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharGroup {
    negated: bool,
    ascii_members: [bool; 128],
}

impl CharGroup {
    pub fn new(negated: bool, members: &str) -> Self {
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

    fn matches(&self, c: char) -> bool {
        c.is_ascii_alphanumeric() && (self.ascii_members[c as usize] ^ self.negated)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledBackreferenceRegex {
    patterns: Vec<Pattern>,
    start_anchor: bool,
    end_anchor: bool,
    literal_prefix: Option<String>,
}

impl CompiledBackreferenceRegex {
    pub fn new(regex: &str) -> Self {
        let (patterns, start_anchor, end_anchor) = Pattern::parse(regex);
        let patterns = normalize_patterns(patterns);
        let literal_prefix = leading_literal_prefix(&patterns);
        Self {
            patterns,
            start_anchor,
            end_anchor,
            literal_prefix,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureSpan {
    start: usize,
    end: usize,
}

type Captures = Vec<Option<CaptureSpan>>;

pub(crate) fn compile_backreference_regex(regex: &str) -> CompiledBackreferenceRegex {
    CompiledBackreferenceRegex::new(regex)
}

pub(crate) fn find_all_backreference_regex_spans_compiled(
    input_line: &str,
    regex: &CompiledBackreferenceRegex,
) -> Vec<RegexMatch> {
    let mut matches = Vec::new();

    if regex.start_anchor {
        if let Some(end) = match_from(regex, input_line, 0) {
            matches.push(RegexMatch { start: 0, end });
        }
        return matches;
    }

    let mut start = 0;
    while start < input_line.len() {
        if let Some(prefix) = regex.literal_prefix.as_deref() {
            let Some(offset) = input_line[start..].find(prefix) else {
                break;
            };
            start += offset;
        }

        if let Some(end) = match_from(regex, input_line, start) {
            matches.push(RegexMatch { start, end });
            start = advance_after_match(input_line, start, end);
        } else {
            let Some(next) = next_char_boundary(input_line, start) else {
                break;
            };
            start = next;
        }
    }

    matches
}

fn leading_literal_prefix(patterns: &[Pattern]) -> Option<String> {
    let mut prefix = String::new();

    for pattern in patterns {
        match pattern {
            Pattern::Literal(ch, count) => {
                let Some(repetitions) = count.fixed_repetitions() else {
                    break;
                };
                prefix.extend(std::iter::repeat_n(*ch, repetitions));
            }
            _ => break,
        }
    }

    (!prefix.is_empty()).then_some(prefix)
}

fn normalize_patterns(patterns: Vec<Pattern>) -> Vec<Pattern> {
    let mut normalized = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        let pattern = match pattern {
            Pattern::Alternation {
                idx,
                alternatives,
                count,
            } => Pattern::Alternation {
                idx,
                alternatives: alternatives.into_iter().map(normalize_patterns).collect(),
                count,
            },
            Pattern::CapturedGroup {
                idx,
                patterns,
                count,
            } => Pattern::CapturedGroup {
                idx,
                patterns: normalize_patterns(patterns),
                count,
            },
            other => other,
        };

        if let Some(previous) = normalized.last_mut() {
            if merge_adjacent_simple_patterns(previous, &pattern) {
                continue;
            }
        }

        normalized.push(pattern);
    }

    normalized
}

fn merge_adjacent_simple_patterns(previous: &mut Pattern, next: &Pattern) -> bool {
    match (previous, next) {
        (Pattern::Literal(left, left_count), Pattern::Literal(right, right_count))
            if left == right =>
        {
            *left_count = left_count.combine(*right_count);
            true
        }
        (Pattern::Digit(left_count), Pattern::Digit(right_count))
        | (Pattern::Alphanumeric(left_count), Pattern::Alphanumeric(right_count))
        | (Pattern::Wildcard(left_count), Pattern::Wildcard(right_count)) => {
            *left_count = left_count.combine(*right_count);
            true
        }
        (
            Pattern::CharGroup(left_group, left_count),
            Pattern::CharGroup(right_group, right_count),
        ) if left_group == right_group => {
            *left_count = left_count.combine(*right_count);
            true
        }
        _ => false,
    }
}

fn match_from(regex: &CompiledBackreferenceRegex, input: &str, start: usize) -> Option<usize> {
    let (end, _) = match_patterns_with_captures(input, &regex.patterns, start, Vec::new())?;

    if !regex.end_anchor || end == input.len() {
        Some(end)
    } else {
        None
    }
}

fn match_patterns_with_captures(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    if patterns.is_empty() {
        return Some((pos, captures));
    }

    let pattern = &patterns[0];
    let remaining = &patterns[1..];

    if pattern.modifies_captures() {
        return match_with_count(
            pattern.count(),
            (pos, captures),
            |current| {
                let (current_pos, current_captures) = current;
                let (next_pos, next_captures) = match_single_with_captures(
                    pattern,
                    input,
                    *current_pos,
                    current_captures.clone(),
                )?;
                Some(((next_pos, next_captures), next_pos != *current_pos))
            },
            |(pos, captures)| {
                match_patterns_with_captures(input, remaining, *pos, captures.clone())
            },
        );
    }

    match_with_count(
        pattern.count(),
        pos,
        |current| {
            let next = match_single_reusing_captures(pattern, input, *current, &captures)?;
            Some((next, next != *current))
        },
        |candidate| match_patterns_with_captures(input, remaining, *candidate, captures.clone()),
    )
}

fn match_with_count<Checkpoint: Clone, Result>(
    count: Count,
    initial: Checkpoint,
    mut advance_once: impl FnMut(&Checkpoint) -> Option<(Checkpoint, bool)>,
    mut try_suffix: impl FnMut(&Checkpoint) -> Option<Result>,
) -> Option<Result> {
    if count.is_exactly_one() {
        let (next, _) = advance_once(&initial)?;
        return try_suffix(&next);
    }

    if count.is_zero_or_one() {
        if let Some((next, _)) = advance_once(&initial) {
            if let Some(result) = try_suffix(&next) {
                return Some(result);
            }
        }
        return try_suffix(&initial);
    }

    match_quantified(count, initial, advance_once, try_suffix)
}

fn match_quantified<Checkpoint: Clone, Result>(
    count: Count,
    initial: Checkpoint,
    mut advance_once: impl FnMut(&Checkpoint) -> Option<(Checkpoint, bool)>,
    mut try_suffix: impl FnMut(&Checkpoint) -> Option<Result>,
) -> Option<Result> {
    let (min, max) = count.repetition_bounds();
    let mut checkpoints = Vec::new();
    let mut current = initial.clone();

    while max.is_none_or(|limit| checkpoints.len() < limit) {
        let Some((next, made_progress)) = advance_once(&current) else {
            break;
        };

        checkpoints.push(next.clone());
        current = next;

        if !made_progress {
            break;
        }
    }

    if checkpoints.len() < min {
        return None;
    }

    for count in (min..=checkpoints.len()).rev() {
        let candidate = if count == 0 {
            &initial
        } else {
            &checkpoints[count - 1]
        };

        if let Some(result) = try_suffix(candidate) {
            return Some(result);
        }
    }

    None
}

fn match_single_reusing_captures(
    pattern: &Pattern,
    input: &str,
    pos: usize,
    captures: &Captures,
) -> Option<usize> {
    match pattern {
        Pattern::Backreference(index) => match_backreference(input, pos, *index, captures),
        _ => match_atom(pattern, input, pos),
    }
}

fn match_single_with_captures(
    pattern: &Pattern,
    input: &str,
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    match pattern {
        Pattern::Alternation {
            alternatives, idx, ..
        } => alternatives.iter().find_map(|alternative| {
            let (end, mut next_captures) =
                match_patterns_with_captures(input, alternative, pos, captures.clone())?;
            set_capture(&mut next_captures, *idx, CaptureSpan { start: pos, end });
            Some((end, next_captures))
        }),
        Pattern::CapturedGroup { patterns, idx, .. } => {
            let (end, mut next_captures) =
                match_patterns_with_captures(input, patterns, pos, captures)?;
            set_capture(&mut next_captures, *idx, CaptureSpan { start: pos, end });
            Some((end, next_captures))
        }
        _ => {
            let next = match_single_reusing_captures(pattern, input, pos, &captures)?;
            Some((next, captures))
        }
    }
}

fn match_atom(pattern: &Pattern, input: &str, pos: usize) -> Option<usize> {
    match pattern {
        Pattern::Literal(literal, _) => match_char(input, pos, |c| c == *literal),
        Pattern::Digit(_) => match_char(input, pos, |c| c.is_ascii_digit()),
        Pattern::Alphanumeric(_) => {
            match_char(input, pos, |c| c.is_ascii_alphanumeric() || c == '_')
        }
        Pattern::Wildcard(_) => {
            let restricted_chars = "\\[](|)";
            match_char(input, pos, |c| !restricted_chars.contains(c))
        }
        Pattern::CharGroup(group, _) => match_char(input, pos, |c| group.matches(c)),
        Pattern::Alternation { .. } | Pattern::CapturedGroup { .. } | Pattern::Backreference(_) => {
            None
        }
    }
}

fn match_backreference(
    input: &str,
    pos: usize,
    index: usize,
    captures: &Captures,
) -> Option<usize> {
    let capture = captures.get(index - 1).copied().flatten()?;
    let matched = &input[capture.start..capture.end];
    input[pos..]
        .starts_with(matched)
        .then_some(pos + matched.len())
}

fn set_capture(captures: &mut Captures, idx: usize, span: CaptureSpan) {
    if captures.len() <= idx {
        captures.resize(idx + 1, None);
    }
    captures[idx] = Some(span);
}

fn match_char(input: &str, pos: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
    let (matched, next) = current_char(input, pos)?;
    pred(matched).then_some(next)
}

fn current_char(input: &str, pos: usize) -> Option<(char, usize)> {
    let matched = input[pos..].chars().next()?;
    Some((matched, pos + matched.len_utf8()))
}

fn next_char_boundary(input: &str, pos: usize) -> Option<usize> {
    current_char(input, pos).map(|(_, next)| next)
}

fn advance_after_match(input: &str, start: usize, end: usize) -> usize {
    if end > start {
        end
    } else {
        next_char_boundary(input, start).unwrap_or(input.len())
    }
}
