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
            Pattern::Alternation { count, .. } => *count,
            Pattern::CapturedGroup { count, .. } => *count,
            Pattern::Backreference(_) => Count::One,
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
    fn repetition_limit(self) -> Option<usize> {
        match self {
            Count::One => Some(1),
            Count::OneOrMore => None,
            Count::ZeroOrOne => Some(1),
            Count::ZeroOrMore => None,
            Count::Exact(n) => Some(n),
            Count::AtLeast(_) => None,
            Count::Range(_, max) => Some(max),
        }
    }

    fn candidate_counts(self, max_available: usize) -> Vec<usize> {
        match self {
            Count::One => (max_available >= 1).then_some(vec![1]).unwrap_or_default(),
            Count::ZeroOrOne => {
                if max_available >= 1 {
                    vec![1]
                } else {
                    vec![0]
                }
            }
            Count::OneOrMore => {
                if max_available == 0 {
                    Vec::new()
                } else {
                    let mut counts = vec![max_available];
                    counts.extend(1..max_available);
                    counts
                }
            }
            Count::ZeroOrMore => {
                if max_available == 0 {
                    vec![0]
                } else {
                    let mut counts = vec![max_available];
                    counts.extend(1..max_available);
                    counts
                }
            }
            Count::Exact(n) => (max_available >= n).then_some(vec![n]).unwrap_or_default(),
            Count::AtLeast(min) => {
                if max_available < min {
                    Vec::new()
                } else {
                    let mut counts = vec![max_available];
                    counts.extend(min..max_available);
                    counts
                }
            }
            Count::Range(min, max) => {
                let capped = max_available.min(max);
                if capped < min {
                    Vec::new()
                } else {
                    (min..=capped).rev().collect()
                }
            }
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

    fn matches(self: &CharGroup, c: char) -> bool {
        c.is_ascii_alphanumeric() && (self.ascii_members[c as usize] ^ self.negated)
    }
}

#[derive(Debug, Clone)]
pub struct CompiledRegex {
    patterns: Vec<Pattern>,
    start_anchor: bool,
    end_anchor: bool,
}

impl CompiledRegex {
    pub fn new(regex: &str) -> Self {
        let (patterns, start_anchor, end_anchor) = Pattern::parse(regex);
        Self {
            patterns,
            start_anchor,
            end_anchor,
        }
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

#[derive(Clone)]
struct MatchState {
    pos: usize,
    captures: Captures,
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
    let captures = Vec::new();
    let (end, _) = match_patterns(input, &regex.patterns, start, &captures)?;
    if !regex.end_anchor || end == input.len() {
        Some(end)
    } else {
        None
    }
}

fn match_patterns(
    input: &str,
    patterns: &[Pattern],
    pos: usize,
    captures: &Captures,
) -> Option<(usize, Captures)> {
    if patterns.is_empty() {
        return Some((pos, captures.clone()));
    }

    let pattern = &patterns[0];
    match pattern.count() {
        Count::One => {
            let state = match_single(pattern, input, pos, captures)?;
            return match_patterns(input, &patterns[1..], state.pos, &state.captures);
        }
        Count::ZeroOrOne => {
            if let Some(state) = match_single(pattern, input, pos, captures) {
                return match_patterns(input, &patterns[1..], state.pos, &state.captures);
            }
            return match_patterns(input, &patterns[1..], pos, captures);
        }
        Count::Exact(1) => {
            let state = match_single(pattern, input, pos, captures)?;
            return match_patterns(input, &patterns[1..], state.pos, &state.captures);
        }
        _ => {}
    }

    let states = collect_match_states(input, pattern, pos, captures);
    let max_available = states.len().saturating_sub(1);

    for count in pattern.count().candidate_counts(max_available) {
        let state = &states[count];
        if let Some((end, final_captures)) =
            match_patterns(input, &patterns[1..], state.pos, &state.captures)
        {
            return Some((end, final_captures));
        }
    }

    None
}

fn collect_match_states(
    input: &str,
    pattern: &Pattern,
    pos: usize,
    captures: &Captures,
) -> Vec<MatchState> {
    let max = pattern.count().repetition_limit();
    let mut states = vec![MatchState {
        pos,
        captures: captures.clone(),
    }];

    while max.is_none_or(|limit| states.len() - 1 < limit) {
        let previous = states.last().unwrap().clone();
        let Some(next) = match_single(pattern, input, previous.pos, &previous.captures) else {
            break;
        };

        if next.pos == previous.pos && max.is_none() {
            break;
        }

        states.push(next);

        if states.last().unwrap().pos == previous.pos {
            break;
        }
    }

    states
}

fn match_single(
    pattern: &Pattern,
    input: &str,
    pos: usize,
    captures: &Captures,
) -> Option<MatchState> {
    match pattern {
        Pattern::Literal(literal, _) => {
            match_char(input, pos, |c| c == *literal).map(|next| MatchState {
                pos: next,
                captures: captures.clone(),
            })
        }
        Pattern::Digit(_) => {
            match_char(input, pos, |c| c.is_ascii_digit()).map(|next| MatchState {
                pos: next,
                captures: captures.clone(),
            })
        }
        Pattern::Alphanumeric(_) => match_char(input, pos, |c| {
            c.is_ascii_alphanumeric() || c == '_'
        })
        .map(|next| MatchState {
            pos: next,
            captures: captures.clone(),
        }),
        Pattern::Wildcard(_) => {
            let restricted_chars = "\\[](|)";
            match_char(input, pos, |c| !restricted_chars.contains(c)).map(|next| MatchState {
                pos: next,
                captures: captures.clone(),
            })
        }
        Pattern::CharGroup(group, _) => {
            match_char(input, pos, |c| group.matches(c)).map(|next| MatchState {
                pos: next,
                captures: captures.clone(),
            })
        }
        Pattern::Alternation {
            alternatives, idx, ..
        } => alternatives.iter().find_map(|alternative| {
            let (end, mut next_captures) = match_patterns(input, alternative, pos, captures)?;
            set_capture(&mut next_captures, *idx, CaptureSpan { start: pos, end });
            Some(MatchState {
                pos: end,
                captures: next_captures,
            })
        }),
        Pattern::CapturedGroup { patterns, idx, .. } => {
            let (end, mut next_captures) = match_patterns(input, patterns, pos, captures)?;
            set_capture(&mut next_captures, *idx, CaptureSpan { start: pos, end });
            Some(MatchState {
                pos: end,
                captures: next_captures,
            })
        }
        Pattern::Backreference(index) => match_backreference(input, pos, *index, captures),
    }
}

fn match_backreference(
    input: &str,
    pos: usize,
    index: usize,
    captures: &Captures,
) -> Option<MatchState> {
    let capture = captures.get(index - 1).copied().flatten()?;
    let matched = &input[capture.start..capture.end];
    input[pos..].starts_with(matched).then_some(MatchState {
        pos: pos + matched.len(),
        captures: captures.clone(),
    })
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
