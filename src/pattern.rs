use crate::parser::Parser;

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
            | Pattern::CharGroup(_, count) => *count,
            Pattern::Alternation { count, .. } | Pattern::CapturedGroup { count, .. } => *count,
            Pattern::Backreference(_) => Count::One,
        }
    }

    fn uses_capture_engine(&self) -> bool {
        matches!(
            self,
            Pattern::Alternation { .. } | Pattern::CapturedGroup { .. } | Pattern::Backreference(_)
        )
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
}

#[derive(Debug, Clone)]
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
pub struct CompiledRegex {
    patterns: Vec<Pattern>,
    start_anchor: bool,
    end_anchor: bool,
    needs_captures: bool,
    literal_prefix: Option<String>,
}

impl CompiledRegex {
    pub fn new(regex: &str) -> Self {
        let (patterns, start_anchor, end_anchor) = Pattern::parse(regex);
        let needs_captures = patterns.iter().any(Pattern::uses_capture_engine);
        let literal_prefix = leading_literal_prefix(&patterns);
        Self {
            patterns,
            start_anchor,
            end_anchor,
            needs_captures,
            literal_prefix,
        }
    }

    pub(crate) fn needs_captures(&self) -> bool {
        self.needs_captures
    }

    #[cfg(test)]
    pub(crate) fn literal_prefix(&self) -> Option<&str> {
        self.literal_prefix.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegexMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureSpan {
    start: usize,
    end: usize,
}

type Captures = Vec<Option<CaptureSpan>>;

pub fn compile_regex(regex: &str) -> CompiledRegex {
    CompiledRegex::new(regex)
}

pub fn find_all_regex_spans_compiled(input_line: &str, regex: &CompiledRegex) -> Vec<RegexMatch> {
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
            Pattern::Literal(ch, count) if count.is_exactly_one() => prefix.push(*ch),
            _ => break,
        }
    }

    (!prefix.is_empty()).then_some(prefix)
}

fn match_from(regex: &CompiledRegex, input: &str, start: usize) -> Option<usize> {
    let end = if regex.needs_captures() {
        let (end, _) = match_patterns_with_captures(input, &regex.patterns, start, Vec::new())?;
        end
    } else {
        match_patterns_without_captures(input, &regex.patterns, start)?
    };

    if !regex.end_anchor || end == input.len() {
        Some(end)
    } else {
        None
    }
}

fn match_patterns_without_captures(input: &str, patterns: &[Pattern], pos: usize) -> Option<usize> {
    if patterns.is_empty() {
        return Some(pos);
    }

    let pattern = &patterns[0];
    let remaining = &patterns[1..];

    match_with_count(
        pattern.count(),
        pos,
        |current| {
            let next = match_atom(pattern, input, *current)?;
            Some((next, next != *current))
        },
        |candidate| match_patterns_without_captures(input, remaining, *candidate),
    )
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
            |candidate| {
                match_patterns_with_captures(input, remaining, candidate.0, candidate.1.clone())
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
