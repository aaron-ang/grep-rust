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
            | Pattern::CharGroup(_, count)
            | Pattern::Alternation { count, .. }
            | Pattern::CapturedGroup { count, .. } => *count,
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
}

impl CompiledRegex {
    pub fn new(regex: &str) -> Self {
        let (patterns, start_anchor, end_anchor) = Pattern::parse(regex);
        let needs_captures = patterns.iter().any(Pattern::uses_capture_engine);
        Self {
            patterns,
            start_anchor,
            end_anchor,
            needs_captures,
        }
    }

    pub(crate) fn needs_captures(&self) -> bool {
        self.needs_captures
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
    match pattern.count() {
        Count::One | Count::Exact(1) => {
            let next = match_single_without_captures(pattern, input, pos)?;
            match_patterns_without_captures(input, &patterns[1..], next)
        }
        Count::ZeroOrOne => {
            if let Some(next) = match_single_without_captures(pattern, input, pos) {
                if let Some(end) = match_patterns_without_captures(input, &patterns[1..], next) {
                    return Some(end);
                }
            }
            match_patterns_without_captures(input, &patterns[1..], pos)
        }
        _ => match_quantified_without_captures(input, patterns, pos),
    }
}

fn match_quantified_without_captures(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
) -> Option<usize> {
    let pattern = &patterns[0];
    let remaining = &patterns[1..];
    let (min, max) = pattern.count().repetition_bounds();
    let mut checkpoints = Vec::new();
    let mut current = pos;

    while max.is_none_or(|limit| checkpoints.len() < limit) {
        let Some(next) = match_single_without_captures(pattern, input, current) else {
            break;
        };

        let made_progress = next != current;
        checkpoints.push(next);
        current = next;

        if !made_progress {
            break;
        }
    }

    if checkpoints.len() < min {
        return None;
    }

    for count in (min..=checkpoints.len()).rev() {
        let candidate_pos = if count == 0 {
            pos
        } else {
            checkpoints[count - 1]
        };
        if let Some(end) = match_patterns_without_captures(input, remaining, candidate_pos) {
            return Some(end);
        }
    }

    None
}

type Captures = Vec<Option<CaptureSpan>>;

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
    if !pattern.modifies_captures() {
        return match_non_modifying_pattern(input, patterns, pos, captures);
    }

    match pattern.count() {
        Count::One | Count::Exact(1) => {
            let (next, next_captures) = match_single_with_captures(pattern, input, pos, captures)?;
            match_patterns_with_captures(input, &patterns[1..], next, next_captures)
        }
        Count::ZeroOrOne => {
            let original = captures;
            if let Some((next, next_captures)) =
                match_single_with_captures(pattern, input, pos, original.clone())
            {
                if let Some(result) =
                    match_patterns_with_captures(input, &patterns[1..], next, next_captures)
                {
                    return Some(result);
                }
            }
            match_patterns_with_captures(input, &patterns[1..], pos, original)
        }
        _ => match_quantified_with_captures(input, patterns, pos, captures),
    }
}

fn match_non_modifying_pattern(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    let pattern = &patterns[0];
    match pattern.count() {
        Count::One | Count::Exact(1) => {
            let next = match_single_reusing_captures(pattern, input, pos, &captures)?;
            match_patterns_with_captures(input, &patterns[1..], next, captures)
        }
        Count::ZeroOrOne => {
            if let Some(next) = match_single_reusing_captures(pattern, input, pos, &captures) {
                if let Some(result) =
                    match_patterns_with_captures(input, &patterns[1..], next, captures.clone())
                {
                    return Some(result);
                }
            }
            match_patterns_with_captures(input, &patterns[1..], pos, captures)
        }
        _ => match_quantified_without_capture_changes(input, patterns, pos, captures),
    }
}

fn match_quantified_without_capture_changes(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    let pattern = &patterns[0];
    let remaining = &patterns[1..];
    let (min, max) = pattern.count().repetition_bounds();
    let mut checkpoints = Vec::new();
    let mut current = pos;

    while max.is_none_or(|limit| checkpoints.len() < limit) {
        let Some(next) = match_single_reusing_captures(pattern, input, current, &captures) else {
            break;
        };

        let made_progress = next != current;
        checkpoints.push(next);
        current = next;

        if !made_progress {
            break;
        }
    }

    if checkpoints.len() < min {
        return None;
    }

    for count in (min..=checkpoints.len()).rev() {
        let candidate_pos = if count == 0 {
            pos
        } else {
            checkpoints[count - 1]
        };
        if let Some(result) =
            match_patterns_with_captures(input, remaining, candidate_pos, captures.clone())
        {
            return Some(result);
        }
    }

    None
}

fn match_quantified_with_captures(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    let pattern = &patterns[0];
    let remaining = &patterns[1..];
    let (min, max) = pattern.count().repetition_bounds();
    let original = captures;
    let mut checkpoints = Vec::new();
    let mut current_pos = pos;
    let mut current_captures = original.clone();

    while max.is_none_or(|limit| checkpoints.len() < limit) {
        let Some((next_pos, next_captures)) =
            match_single_with_captures(pattern, input, current_pos, current_captures)
        else {
            break;
        };

        let made_progress = next_pos != current_pos;
        checkpoints.push((next_pos, next_captures.clone()));
        current_pos = next_pos;
        current_captures = next_captures;

        if !made_progress {
            break;
        }
    }

    if checkpoints.len() < min {
        return None;
    }

    for count in (min..=checkpoints.len()).rev() {
        let (candidate_pos, candidate_captures) = if count == 0 {
            (pos, original.clone())
        } else {
            checkpoints[count - 1].clone()
        };

        if let Some(result) =
            match_patterns_with_captures(input, remaining, candidate_pos, candidate_captures)
        {
            return Some(result);
        }
    }

    None
}

fn match_single_without_captures(pattern: &Pattern, input: &str, pos: usize) -> Option<usize> {
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
            unreachable!("capture-aware patterns must use the capture engine")
        }
    }
}

fn match_single_reusing_captures(
    pattern: &Pattern,
    input: &str,
    pos: usize,
    captures: &Captures,
) -> Option<usize> {
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
        Pattern::Backreference(index) => match_backreference(input, pos, *index, captures),
        Pattern::Alternation { .. } | Pattern::CapturedGroup { .. } => {
            unreachable!("capture-modifying patterns need owned captures")
        }
    }
}

fn match_single_with_captures(
    pattern: &Pattern,
    input: &str,
    pos: usize,
    captures: Captures,
) -> Option<(usize, Captures)> {
    match pattern {
        Pattern::Literal(_, _)
        | Pattern::Digit(_)
        | Pattern::Alphanumeric(_)
        | Pattern::Wildcard(_)
        | Pattern::CharGroup(_, _)
        | Pattern::Backreference(_) => {
            let next = match_single_reusing_captures(pattern, input, pos, &captures)?;
            Some((next, captures))
        }
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
