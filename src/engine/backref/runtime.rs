use crate::engine::RegexMatch;

use super::ast::{CaptureSpan, CharGroup, Count, Pattern};

#[derive(Debug)]
pub(super) struct CompiledBackreferenceRegex {
    plan: BackreferencePlan,
}

#[derive(Debug)]
struct BackreferencePlan {
    instructions: Vec<Instruction>,
    referenced_capture_count: usize,
    start_anchor: bool,
    end_anchor: bool,
    search_hints: SearchHints,
    fast_path: Option<FastPath>,
}

#[derive(Debug)]
struct SearchHints {
    anchor: Option<AnchorLiteral>,
    start_predicate: StartPredicate,
    candidate_strategy: CandidateStrategy,
}

#[derive(Debug)]
enum FastPath {
    SingleCaptureLiteralBackref(SingleCaptureLiteralBackref),
    TwoPartReplayBackref(TwoPartReplayBackref),
}

#[derive(Debug)]
struct SingleCaptureLiteralBackref {
    matcher: RepeatedAtomMatcher,
    separator: String,
}

#[derive(Debug)]
struct TwoPartReplayBackref {
    first_matcher: RepeatedAtomMatcher,
    middle_separator: String,
    second_matcher: RepeatedAtomMatcher,
    separator: String,
}

#[derive(Debug)]
enum SimpleAtom {
    Literal(char),
    Digit,
    Word,
    CharGroup(CharGroup),
}

#[derive(Debug)]
struct AnchorLiteral {
    text: String,
    prefix_width: Option<usize>,
}

#[derive(Debug, PartialEq, Eq)]
enum StartPredicate {
    Any,
    Literal(char),
    Digit,
    Word,
    Wildcard,
    CharGroup(CharGroup),
}

#[derive(Debug, Clone, Copy)]
enum CandidateStrategy {
    FixedPrefixAnchor,
    VariablePrefixLiteralAnchor,
    StartPredicateScan,
}

#[derive(Debug)]
struct RepeatedAtomMatcher {
    atom: SimpleAtom,
    min: usize,
    max: Option<usize>,
}

#[derive(Debug)]
enum Instruction {
    ConsumeLiteral(char),
    ConsumeDigit,
    ConsumeWord,
    ConsumeWildcard,
    ConsumeCharGroup(CharGroup),
    Split { preferred: usize, fallback: usize },
    Jump(usize),
    SaveCaptureStart(usize),
    SaveCaptureEnd(usize),
    MatchBackref(usize),
    MatchEnd,
}

#[derive(Clone)]
struct VmState {
    pc: usize,
    pos: usize,
    captures: Vec<Option<CaptureSpan>>,
    epsilon_trace: Vec<(usize, usize)>,
}

#[derive(Default)]
struct AnchorCollector {
    best: Option<(String, Option<usize>)>,
}

impl CompiledBackreferenceRegex {
    pub(super) fn new(regex: &str) -> Self {
        let (patterns, start_anchor, end_anchor) = Pattern::parse(regex);
        Self {
            plan: BackreferencePlan::compile(
                &normalize_patterns(patterns),
                start_anchor,
                end_anchor,
            ),
        }
    }

    pub(super) fn find_all(&self, input: &str) -> Vec<RegexMatch> {
        self.plan.find_all(input)
    }
}

impl SimpleAtom {
    fn matches_at(&self, input: &str, pos: usize) -> bool {
        match self {
            SimpleAtom::Literal(ch) => matches_literal(input, pos, *ch),
            SimpleAtom::Digit => matches_digit(input, pos),
            SimpleAtom::Word => matches_word(input, pos),
            SimpleAtom::CharGroup(group) => matches_group(input, pos, group),
        }
    }
}

impl StartPredicate {
    fn matches_at(&self, input: &str, pos: usize) -> bool {
        match self {
            StartPredicate::Any => pos <= input.len(),
            StartPredicate::Literal(ch) => matches_literal(input, pos, *ch),
            StartPredicate::Digit => matches_digit(input, pos),
            StartPredicate::Word => matches_word(input, pos),
            StartPredicate::Wildcard => {
                match_char(input, pos, |current| !r"\[](|)".contains(current)).is_some()
            }
            StartPredicate::CharGroup(group) => matches_group(input, pos, group),
        }
    }
}

impl RepeatedAtomMatcher {
    fn from_pattern(pattern: &Pattern) -> Option<Self> {
        let count = pattern.count();
        let (min, max) = count.repetition_bounds();
        if min == 0 {
            return None;
        }

        let atom = match pattern {
            Pattern::Literal(ch, _) => Some(SimpleAtom::Literal(*ch)),
            Pattern::Digit(_) => Some(SimpleAtom::Digit),
            Pattern::Alphanumeric(_) => Some(SimpleAtom::Word),
            Pattern::CharGroup(group, _) => Some(SimpleAtom::CharGroup(group.clone())),
            Pattern::Wildcard(_)
            | Pattern::Alternation { .. }
            | Pattern::CapturedGroup { .. }
            | Pattern::Backreference(_) => None,
        }?;

        Some(Self { atom, min, max })
    }

    fn fixed_byte_width(&self) -> Option<usize> {
        let repetitions = self.max.filter(|max| *max == self.min)?;
        Some(self.unit_byte_width() * repetitions)
    }

    fn unit_byte_width(&self) -> usize {
        match &self.atom {
            SimpleAtom::Literal(ch) => ch.len_utf8(),
            SimpleAtom::Digit | SimpleAtom::Word | SimpleAtom::CharGroup(_) => 1,
        }
    }

    fn byte_len_for_count(&self, count: usize) -> usize {
        self.unit_byte_width() * count
    }

    fn matches_entire(&self, input: &str) -> bool {
        let mut current = 0;
        let mut count = 0;

        while current < input.len() {
            if !self.atom.matches_at(input, current) {
                return false;
            }
            let Some(next) = next_char_boundary(input, current) else {
                return false;
            };
            current = next;
            count += 1;
            if self.max.is_some_and(|max| count > max) {
                return false;
            }
        }

        count >= self.min
    }

    fn count_backward(&self, input: &str, scan_start: usize, end: usize) -> usize {
        let mut count = 0;
        let mut current = end;

        while self.max.is_none_or(|limit| count < limit) {
            let Some(previous) = previous_char_boundary(input, current) else {
                break;
            };
            if previous < scan_start || !self.atom.matches_at(input, previous) {
                break;
            }
            count += 1;
            current = previous;
        }

        count
    }

    fn count_forward(&self, input: &str, start: usize) -> usize {
        let mut count = 0;
        let mut current = start;

        while self.max.is_none_or(|limit| count < limit) {
            if !self.atom.matches_at(input, current) {
                break;
            }
            let Some(next) = next_char_boundary(input, current) else {
                break;
            };
            count += 1;
            current = next;
        }

        count
    }
}

impl BackreferencePlan {
    fn compile(patterns: &[Pattern], start_anchor: bool, end_anchor: bool) -> Self {
        let referenced_groups = referenced_groups(patterns);
        let group_slots = build_group_slots(&referenced_groups);
        Self {
            instructions: BackreferenceCompiler::compile(patterns, &group_slots),
            referenced_capture_count: referenced_groups.len(),
            start_anchor,
            end_anchor,
            search_hints: SearchHints::analyze(patterns, start_anchor),
            fast_path: detect_fast_path(patterns),
        }
    }

    fn find_all(&self, input: &str) -> Vec<RegexMatch> {
        let mut matches = Vec::new();

        if self.start_anchor {
            if let Some(found) = self.match_from(input, 0) {
                matches.push(found);
            }
            return matches;
        }

        let mut scan_start = 0;
        while scan_start < input.len() {
            let Some(found) = self.find_next_match(input, scan_start) else {
                break;
            };
            matches.push(found);
            scan_start = advance_after_match(input, found.start, found.end);
        }

        matches
    }

    fn find_next_match(&self, input: &str, scan_start: usize) -> Option<RegexMatch> {
        if self.fast_path.is_some() {
            return self.find_fast_path_match(input, scan_start);
        }

        self.search_hints
            .visit_candidates(self, input, scan_start, |candidate| {
                self.match_at(input, candidate)
            })
    }

    fn match_from(&self, input: &str, start: usize) -> Option<RegexMatch> {
        if self.fast_path.is_some() {
            let found = self.find_fast_path_match(input, start)?;
            return (found.start == start).then_some(found);
        }

        self.match_at(input, start)
    }

    fn match_at(&self, input: &str, start: usize) -> Option<RegexMatch> {
        let end = self.execute(input, start)?;
        (!self.end_anchor || end == input.len()).then_some(RegexMatch { start, end })
    }

    fn find_fast_path_match(&self, input: &str, scan_start: usize) -> Option<RegexMatch> {
        let fast_path = self.fast_path.as_ref()?;
        match fast_path {
            FastPath::SingleCaptureLiteralBackref(fast_path) => {
                self.find_single_capture_literal_backref(input, scan_start, fast_path)
            }
            FastPath::TwoPartReplayBackref(fast_path) => {
                self.find_two_part_replay_backref(input, scan_start, fast_path)
            }
        }
    }

    fn find_single_capture_literal_backref(
        &self,
        input: &str,
        scan_start: usize,
        fast_path: &SingleCaptureLiteralBackref,
    ) -> Option<RegexMatch> {
        let mut search_from = scan_start;

        while let Some(separator_start) =
            find_literal_from(input, search_from, &fast_path.separator)
        {
            let separator_end = separator_start + fast_path.separator.len();

            if let Some(found) = self.match_single_capture_literal_backref_at(
                input,
                scan_start,
                separator_start,
                separator_end,
                &fast_path.matcher,
            ) {
                return Some(found);
            }

            let Some(next) = next_char_boundary(input, separator_start) else {
                break;
            };
            search_from = next;
        }

        None
    }

    fn match_single_capture_literal_backref_at(
        &self,
        input: &str,
        scan_start: usize,
        separator_start: usize,
        separator_end: usize,
        matcher: &RepeatedAtomMatcher,
    ) -> Option<RegexMatch> {
        if let Some(width) = matcher.fixed_byte_width() {
            let start = separator_start.checked_sub(width)?;
            let end = separator_end.checked_add(width)?;
            if start < scan_start || end > input.len() || !input.is_char_boundary(start) {
                return None;
            }
            let capture = input.get(start..separator_start)?;
            if !matcher.matches_entire(capture) {
                return None;
            }
            let found = (input.get(separator_end..end) == Some(capture))
                .then_some(RegexMatch { start, end })?;
            return (!self.end_anchor || found.end == input.len()).then_some(found);
        }

        let left_count = matcher.count_backward(input, scan_start, separator_start);
        let right_count = matcher.count_forward(input, separator_end);
        let max_count = left_count.min(right_count);

        if max_count < matcher.min {
            return None;
        }

        let upper = matcher.max.unwrap_or(max_count).min(max_count);
        let unit_width = matcher.unit_byte_width();
        for count in (matcher.min..=upper).rev() {
            let byte_len = count * unit_width;
            let start = separator_start.checked_sub(byte_len)?;
            let end = separator_end.checked_add(byte_len)?;
            let capture = input.get(start..separator_start)?;
            if input.get(separator_end..end) == Some(capture) {
                let found = RegexMatch { start, end };
                if !self.end_anchor || found.end == input.len() {
                    return Some(found);
                }
            }
        }

        None
    }

    fn find_two_part_replay_backref(
        &self,
        input: &str,
        scan_start: usize,
        fast_path: &TwoPartReplayBackref,
    ) -> Option<RegexMatch> {
        let mut search_from = scan_start;

        while let Some(separator_start) =
            find_literal_from(input, search_from, &fast_path.separator)
        {
            let separator_end = separator_start + fast_path.separator.len();

            if let Some(found) = self.match_two_part_replay_backref_at(
                input,
                scan_start,
                separator_start,
                separator_end,
                fast_path,
            ) {
                return Some(found);
            }

            let Some(next) = next_char_boundary(input, separator_start) else {
                break;
            };
            search_from = next;
        }

        None
    }

    fn match_two_part_replay_backref_at(
        &self,
        input: &str,
        scan_start: usize,
        separator_start: usize,
        separator_end: usize,
        fast_path: &TwoPartReplayBackref,
    ) -> Option<RegexMatch> {
        let second_available =
            fast_path
                .second_matcher
                .count_backward(input, scan_start, separator_start);
        let second_upper = fast_path
            .second_matcher
            .max
            .unwrap_or(second_available)
            .min(second_available);

        if second_upper < fast_path.second_matcher.min {
            return None;
        }

        for second_count in (fast_path.second_matcher.min..=second_upper).rev() {
            let second_byte_len = fast_path.second_matcher.byte_len_for_count(second_count);
            let Some(second_start) = separator_start.checked_sub(second_byte_len) else {
                continue;
            };
            let Some(first_end) = second_start.checked_sub(fast_path.middle_separator.len()) else {
                continue;
            };
            if input.get(first_end..second_start) != Some(fast_path.middle_separator.as_str()) {
                continue;
            }

            let first_available = fast_path
                .first_matcher
                .count_backward(input, scan_start, first_end);
            let first_upper = fast_path
                .first_matcher
                .max
                .unwrap_or(first_available)
                .min(first_available);

            if first_upper < fast_path.first_matcher.min {
                continue;
            }

            for first_count in (fast_path.first_matcher.min..=first_upper).rev() {
                let first_byte_len = fast_path.first_matcher.byte_len_for_count(first_count);
                let Some(start) = first_end.checked_sub(first_byte_len) else {
                    continue;
                };
                if start < scan_start || !input.is_char_boundary(start) {
                    continue;
                }

                let Some(left_side) = input.get(start..separator_start) else {
                    continue;
                };
                let Some(end) = separator_end.checked_add(left_side.len()) else {
                    continue;
                };
                let found = (input.get(separator_end..end) == Some(left_side))
                    .then_some(RegexMatch { start, end })?;
                if !self.end_anchor || found.end == input.len() {
                    return Some(found);
                }
            }
        }

        None
    }

    fn execute(&self, input: &str, start: usize) -> Option<usize> {
        let mut stack = vec![VmState::new(self.referenced_capture_count, start)];

        while let Some(mut state) = stack.pop() {
            loop {
                let instruction = self.instructions.get(state.pc)?;

                if instruction.is_epsilon() && state.has_visited_epsilon() {
                    break;
                }

                if instruction.is_epsilon() {
                    state.mark_epsilon();
                }

                match instruction {
                    Instruction::ConsumeLiteral(ch) => {
                        let Some(next) = match_char(input, state.pos, |current| current == *ch)
                        else {
                            break;
                        };
                        state.advance(next);
                    }
                    Instruction::ConsumeDigit => {
                        let Some(next) =
                            match_char(input, state.pos, |current| current.is_ascii_digit())
                        else {
                            break;
                        };
                        state.advance(next);
                    }
                    Instruction::ConsumeWord => {
                        let Some(next) = match_char(input, state.pos, |current| {
                            current.is_ascii_alphanumeric() || current == '_'
                        }) else {
                            break;
                        };
                        state.advance(next);
                    }
                    Instruction::ConsumeWildcard => {
                        let Some(next) =
                            match_char(input, state.pos, |current| !r"\[](|)".contains(current))
                        else {
                            break;
                        };
                        state.advance(next);
                    }
                    Instruction::ConsumeCharGroup(group) => {
                        let Some(next) =
                            match_char(input, state.pos, |current| group.matches(current))
                        else {
                            break;
                        };
                        state.advance(next);
                    }
                    Instruction::Split {
                        preferred,
                        fallback,
                    } => {
                        stack.push(state.fork(*fallback));
                        state.pc = *preferred;
                    }
                    Instruction::Jump(target) => state.pc = *target,
                    Instruction::SaveCaptureStart(slot) => {
                        state.captures[*slot] = Some(CaptureSpan {
                            start: state.pos,
                            end: state.pos,
                        });
                        state.pc += 1;
                    }
                    Instruction::SaveCaptureEnd(slot) => {
                        let start = state.captures[*slot].map_or(state.pos, |span| span.start);
                        state.captures[*slot] = Some(CaptureSpan {
                            start,
                            end: state.pos,
                        });
                        state.pc += 1;
                    }
                    Instruction::MatchBackref(slot) => {
                        let Some(capture) = state.captures[*slot] else {
                            break;
                        };
                        let matched = &input[capture.start..capture.end];
                        let Some(rest) = input.get(state.pos..) else {
                            break;
                        };
                        if !rest.starts_with(matched) {
                            break;
                        }
                        state.pos += matched.len();
                        state.pc += 1;
                        if !matched.is_empty() {
                            state.epsilon_trace.clear();
                        }
                    }
                    Instruction::MatchEnd => return Some(state.pos),
                }
            }
        }

        None
    }
}

impl SearchHints {
    fn analyze(patterns: &[Pattern], start_anchor: bool) -> Self {
        let anchor = extract_anchor_literal(patterns);
        let start_predicate = if start_anchor {
            StartPredicate::Any
        } else {
            first_required_start_predicate(patterns).unwrap_or(StartPredicate::Any)
        };
        let candidate_strategy =
            candidate_strategy_for(start_anchor, anchor.as_ref(), patterns, &start_predicate);

        Self {
            anchor,
            start_predicate,
            candidate_strategy,
        }
    }

    fn visit_candidates<T>(
        &self,
        plan: &BackreferencePlan,
        input: &str,
        scan_start: usize,
        mut visit: impl FnMut(usize) -> Option<T>,
    ) -> Option<T> {
        if plan.start_anchor {
            return (scan_start == 0).then_some(0).and_then(visit);
        }

        let mut last_candidate = None;

        match self.candidate_strategy {
            CandidateStrategy::FixedPrefixAnchor => {
                let anchor = self.anchor.as_ref()?;
                let prefix_width = anchor.prefix_width?;
                visit_anchor_hits(input, scan_start, &anchor.text, |anchor_start| {
                    let candidate = anchor_start.checked_sub(prefix_width)?;
                    if candidate < scan_start || !input.is_char_boundary(candidate) {
                        return None;
                    }
                    if last_candidate.replace(candidate) == Some(candidate) {
                        return None;
                    }
                    visit(candidate)
                })
            }
            CandidateStrategy::VariablePrefixLiteralAnchor => {
                let anchor = self.anchor.as_ref()?;
                visit_anchor_hits(input, scan_start, &anchor.text, |anchor_start| {
                    visit_bounded_backward_candidates(
                        input,
                        scan_start,
                        anchor_start,
                        &self.start_predicate,
                        |candidate| {
                            if last_candidate.replace(candidate) == Some(candidate) {
                                return None;
                            }
                            visit(candidate)
                        },
                    )
                })
            }
            CandidateStrategy::StartPredicateScan => {
                visit_scan_candidates(input, scan_start, input.len(), &self.start_predicate, visit)
            }
        }
    }
}

impl BackreferenceCompiler<'_> {
    fn compile(patterns: &[Pattern], group_slots: &[Option<usize>]) -> Vec<Instruction> {
        let mut compiler = BackreferenceCompiler {
            instructions: Vec::new(),
            group_slots,
        };
        compiler.compile_sequence(patterns);
        compiler.instructions.push(Instruction::MatchEnd);
        compiler.instructions
    }

    fn compile_sequence(&mut self, patterns: &[Pattern]) {
        for pattern in patterns {
            self.compile_pattern(pattern);
        }
    }

    fn compile_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Literal(ch, count) => {
                self.compile_count(*count, |compiler| {
                    compiler.emit(Instruction::ConsumeLiteral(*ch));
                });
            }
            Pattern::Digit(count) => {
                self.compile_count(*count, |compiler| compiler.emit(Instruction::ConsumeDigit));
            }
            Pattern::Alphanumeric(count) => {
                self.compile_count(*count, |compiler| compiler.emit(Instruction::ConsumeWord));
            }
            Pattern::Wildcard(count) => {
                self.compile_count(*count, |compiler| {
                    compiler.emit(Instruction::ConsumeWildcard);
                });
            }
            Pattern::CharGroup(group, count) => {
                let group = group.clone();
                self.compile_count(*count, move |compiler| {
                    compiler.emit(Instruction::ConsumeCharGroup(group.clone()));
                });
            }
            Pattern::Backreference(group) => {
                let slot = self.group_slots[*group - 1]
                    .unwrap_or_else(|| panic!("missing slot for referenced capture group {group}"));
                self.emit(Instruction::MatchBackref(slot));
            }
            Pattern::CapturedGroup {
                idx,
                patterns,
                count,
            } => {
                let slot = self.group_slots.get(*idx).copied().flatten();
                self.compile_count(*count, |compiler| {
                    if let Some(slot) = slot {
                        compiler.emit(Instruction::SaveCaptureStart(slot));
                    }
                    compiler.compile_sequence(patterns);
                    if let Some(slot) = slot {
                        compiler.emit(Instruction::SaveCaptureEnd(slot));
                    }
                });
            }
            Pattern::Alternation {
                idx,
                alternatives,
                count,
            } => {
                let slot = self.group_slots.get(*idx).copied().flatten();
                self.compile_count(*count, |compiler| {
                    if let Some(slot) = slot {
                        compiler.emit(Instruction::SaveCaptureStart(slot));
                    }
                    compiler.compile_alternatives(alternatives);
                    if let Some(slot) = slot {
                        compiler.emit(Instruction::SaveCaptureEnd(slot));
                    }
                });
            }
        }
    }

    fn compile_count(
        &mut self,
        count: Count,
        mut emit_body: impl FnMut(&mut BackreferenceCompiler<'_>),
    ) {
        match count {
            Count::One => emit_body(self),
            Count::ZeroOrOne => self.compile_optional(&mut emit_body),
            Count::OneOrMore => self.compile_one_or_more(&mut emit_body),
            Count::ZeroOrMore => self.compile_zero_or_more(&mut emit_body),
            Count::Exact(times) => {
                for _ in 0..times {
                    emit_body(self);
                }
            }
            Count::AtLeast(min) => {
                for _ in 0..min {
                    emit_body(self);
                }
                self.compile_zero_or_more(&mut emit_body);
            }
            Count::Range(min, max) => {
                for _ in 0..min {
                    emit_body(self);
                }
                for _ in min..max {
                    self.compile_optional(&mut emit_body);
                }
            }
        }
    }

    fn compile_optional(&mut self, emit_body: &mut impl FnMut(&mut BackreferenceCompiler<'_>)) {
        let split_idx = self.emit_split_placeholder();
        let body_start = self.instructions.len();
        emit_body(self);
        self.patch_split(split_idx, body_start, self.instructions.len());
    }

    fn compile_one_or_more(&mut self, emit_body: &mut impl FnMut(&mut BackreferenceCompiler<'_>)) {
        let body_start = self.instructions.len();
        emit_body(self);
        let split_idx = self.emit_split_placeholder();
        self.patch_split(split_idx, body_start, self.instructions.len());
    }

    fn compile_zero_or_more(&mut self, emit_body: &mut impl FnMut(&mut BackreferenceCompiler<'_>)) {
        let split_idx = self.emit_split_placeholder();
        let body_start = self.instructions.len();
        emit_body(self);
        self.emit(Instruction::Jump(split_idx));
        self.patch_split(split_idx, body_start, self.instructions.len());
    }

    fn compile_alternatives(&mut self, alternatives: &[Vec<Pattern>]) {
        if let Some((first, remaining)) = alternatives.split_first() {
            if remaining.is_empty() {
                self.compile_sequence(first);
                return;
            }

            let split_idx = self.emit_split_placeholder();
            let first_start = self.instructions.len();
            self.compile_sequence(first);
            let jump_idx = self.emit_jump_placeholder();
            let fallback_start = self.instructions.len();
            self.compile_alternatives(remaining);
            self.patch_split(split_idx, first_start, fallback_start);
            self.patch_jump(jump_idx, self.instructions.len());
        }
    }

    fn emit(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }

    fn emit_split_placeholder(&mut self) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(Instruction::Split {
            preferred: 0,
            fallback: 0,
        });
        idx
    }

    fn emit_jump_placeholder(&mut self) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(Instruction::Jump(0));
        idx
    }

    fn patch_split(&mut self, idx: usize, preferred: usize, fallback: usize) {
        self.instructions[idx] = Instruction::Split {
            preferred,
            fallback,
        };
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        self.instructions[idx] = Instruction::Jump(target);
    }
}

impl AnchorCollector {
    fn visit_sequence(
        &mut self,
        patterns: &[Pattern],
        prefix_width: Option<usize>,
    ) -> Option<usize> {
        let mut current_prefix = prefix_width;
        let mut current_run = String::new();
        let mut run_prefix = current_prefix;

        for pattern in patterns {
            if let Some((ch, repetitions)) = literal_run_atom(pattern) {
                if current_run.is_empty() {
                    run_prefix = current_prefix;
                }
                current_run.extend(std::iter::repeat_n(ch, repetitions));
                self.consider_run(&current_run, run_prefix);
                current_prefix = combine_widths(current_prefix, Some(repetitions));
                continue;
            }

            current_run.clear();

            if let Pattern::CapturedGroup {
                patterns, count, ..
            } = pattern
            {
                if count.is_exactly_one() {
                    self.visit_sequence(patterns, current_prefix);
                }
            }

            current_prefix = combine_widths(current_prefix, fixed_width(pattern));
        }

        current_prefix
    }

    fn consider_run(&mut self, run: &str, prefix_width: Option<usize>) {
        let should_replace = self
            .best
            .as_ref()
            .is_none_or(|(best, _)| run.len() > best.len());

        if should_replace {
            self.best = Some((run.to_string(), prefix_width));
        }
    }
}

impl Instruction {
    fn is_epsilon(&self) -> bool {
        matches!(
            self,
            Instruction::Split { .. }
                | Instruction::Jump(_)
                | Instruction::SaveCaptureStart(_)
                | Instruction::SaveCaptureEnd(_)
                | Instruction::MatchBackref(_)
                | Instruction::MatchEnd
        )
    }
}

impl VmState {
    fn new(referenced_capture_count: usize, start: usize) -> Self {
        Self {
            pc: 0,
            pos: start,
            captures: vec![None; referenced_capture_count],
            epsilon_trace: Vec::new(),
        }
    }

    fn has_visited_epsilon(&self) -> bool {
        self.epsilon_trace
            .iter()
            .any(|&(pc, pos)| pc == self.pc && pos == self.pos)
    }

    fn mark_epsilon(&mut self) {
        self.epsilon_trace.push((self.pc, self.pos));
    }

    fn advance(&mut self, next_pos: usize) {
        self.pos = next_pos;
        self.pc += 1;
        self.epsilon_trace.clear();
    }

    fn fork(&self, pc: usize) -> Self {
        let mut alternate = self.clone();
        alternate.pc = pc;
        alternate
    }
}

struct BackreferenceCompiler<'a> {
    instructions: Vec<Instruction>,
    group_slots: &'a [Option<usize>],
}

fn normalize_patterns(patterns: Vec<Pattern>) -> Vec<Pattern> {
    let mut normalized: Vec<Pattern> = Vec::with_capacity(patterns.len());

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
            if merge_adjacent_simple(previous, &pattern) {
                continue;
            }
        }

        normalized.push(pattern);
    }

    normalized
}

fn merge_adjacent_simple(previous: &mut Pattern, next: &Pattern) -> bool {
    match (previous, next) {
        (Pattern::Literal(left, left_count), Pattern::Literal(right, right_count))
            if left == right =>
        {
            *left_count = left_count.combine(right_count);
            true
        }
        (Pattern::Digit(left_count), Pattern::Digit(right_count))
        | (Pattern::Alphanumeric(left_count), Pattern::Alphanumeric(right_count))
        | (Pattern::Wildcard(left_count), Pattern::Wildcard(right_count)) => {
            *left_count = left_count.combine(right_count);
            true
        }
        (
            Pattern::CharGroup(left_group, left_count),
            Pattern::CharGroup(right_group, right_count),
        ) if left_group == right_group => {
            *left_count = left_count.combine(right_count);
            true
        }
        _ => false,
    }
}

fn referenced_groups(patterns: &[Pattern]) -> Vec<usize> {
    let mut referenced = Vec::new();
    collect_referenced_groups(patterns, &mut referenced);
    referenced.sort_unstable();
    referenced.dedup();
    referenced
}

fn collect_referenced_groups(patterns: &[Pattern], referenced: &mut Vec<usize>) {
    for pattern in patterns {
        match pattern {
            Pattern::Alternation { alternatives, .. } => {
                for alternative in alternatives {
                    collect_referenced_groups(alternative, referenced);
                }
            }
            Pattern::CapturedGroup { patterns, .. } => {
                collect_referenced_groups(patterns, referenced);
            }
            Pattern::Backreference(group) => referenced.push(group - 1),
            Pattern::Literal(_, _)
            | Pattern::Digit(_)
            | Pattern::Alphanumeric(_)
            | Pattern::Wildcard(_)
            | Pattern::CharGroup(_, _) => {}
        }
    }
}

fn build_group_slots(referenced_groups: &[usize]) -> Vec<Option<usize>> {
    let Some(max_group) = referenced_groups.iter().copied().max() else {
        return Vec::new();
    };

    let mut slots = vec![None; max_group + 1];
    for (slot, group_idx) in referenced_groups.iter().copied().enumerate() {
        slots[group_idx] = Some(slot);
    }
    slots
}

fn detect_fast_path(patterns: &[Pattern]) -> Option<FastPath> {
    detect_single_capture_literal_backref(patterns)
        .map(FastPath::SingleCaptureLiteralBackref)
        .or_else(|| detect_two_part_replay_backref(patterns).map(FastPath::TwoPartReplayBackref))
}

fn detect_single_capture_literal_backref(
    patterns: &[Pattern],
) -> Option<SingleCaptureLiteralBackref> {
    let (first, remaining) = patterns.split_first()?;
    let capture_pattern = captured_single_pattern(first, 0)?;
    let matcher = RepeatedAtomMatcher::from_pattern(capture_pattern)?;
    let (separator, literal_count) = leading_literal_sequence(remaining)?;
    let tail = &remaining[literal_count..];

    if tail.len() == 1 && matches!(tail[0], Pattern::Backreference(1)) {
        Some(SingleCaptureLiteralBackref { matcher, separator })
    } else {
        None
    }
}

fn detect_two_part_replay_backref(patterns: &[Pattern]) -> Option<TwoPartReplayBackref> {
    detect_direct_two_part_replay_backref(patterns)
        .or_else(|| detect_group_replay_backref(patterns))
}

fn detect_direct_two_part_replay_backref(patterns: &[Pattern]) -> Option<TwoPartReplayBackref> {
    let (first, remaining) = patterns.split_first()?;
    let first_pattern = captured_single_pattern(first, 0)?;
    let (middle_separator, middle_count) = leading_literal_sequence(remaining)?;
    let remaining = &remaining[middle_count..];
    let (second, remaining) = remaining.split_first()?;
    let second_pattern = captured_single_pattern(second, 1)?;
    let (separator, separator_count) = leading_literal_sequence(remaining)?;
    let tail = &remaining[separator_count..];

    if !matches_two_part_replay_tail(tail, &middle_separator) {
        return None;
    }

    Some(TwoPartReplayBackref {
        first_matcher: RepeatedAtomMatcher::from_pattern(first_pattern)?,
        middle_separator,
        second_matcher: RepeatedAtomMatcher::from_pattern(second_pattern)?,
        separator,
    })
}

fn detect_group_replay_backref(patterns: &[Pattern]) -> Option<TwoPartReplayBackref> {
    let (first, remaining) = patterns.split_first()?;
    let Pattern::CapturedGroup {
        idx,
        patterns: inner,
        count,
    } = first
    else {
        return None;
    };

    if *idx != 0 || !count.is_exactly_one() {
        return None;
    }

    let (first_pattern, middle_separator, second_pattern) = extract_two_part_capture(inner, 1, 2)?;
    let (separator, separator_count) = leading_literal_sequence(remaining)?;
    let tail = &remaining[separator_count..];

    if tail.len() != 1 || !matches!(tail[0], Pattern::Backreference(1)) {
        return None;
    }

    Some(TwoPartReplayBackref {
        first_matcher: RepeatedAtomMatcher::from_pattern(first_pattern)?,
        middle_separator,
        second_matcher: RepeatedAtomMatcher::from_pattern(second_pattern)?,
        separator,
    })
}

fn extract_two_part_capture(
    patterns: &[Pattern],
    first_idx: usize,
    second_idx: usize,
) -> Option<(&Pattern, String, &Pattern)> {
    let (first, remaining) = patterns.split_first()?;
    let first_pattern = captured_single_pattern(first, first_idx)?;
    let (middle_separator, middle_count) = leading_literal_sequence(remaining)?;
    let remaining = &remaining[middle_count..];
    let (second, tail) = remaining.split_first()?;
    let second_pattern = captured_single_pattern(second, second_idx)?;

    tail.is_empty()
        .then_some((first_pattern, middle_separator, second_pattern))
}

fn captured_single_pattern(pattern: &Pattern, expected_idx: usize) -> Option<&Pattern> {
    let Pattern::CapturedGroup {
        idx,
        patterns,
        count,
    } = pattern
    else {
        return None;
    };

    if *idx != expected_idx || !count.is_exactly_one() {
        return None;
    }

    let [single] = patterns.as_slice() else {
        return None;
    };

    Some(single)
}

fn matches_two_part_replay_tail(patterns: &[Pattern], middle_separator: &str) -> bool {
    let [Pattern::Backreference(1), rest @ ..] = patterns else {
        return false;
    };
    let Some((literal, consumed)) = leading_literal_sequence(rest) else {
        return false;
    };

    literal == middle_separator
        && rest.len() == consumed + 1
        && matches!(rest[consumed], Pattern::Backreference(2))
}

fn extract_anchor_literal(patterns: &[Pattern]) -> Option<AnchorLiteral> {
    let mut collector = AnchorCollector::default();
    collector.visit_sequence(patterns, Some(0));
    collector
        .best
        .as_ref()
        .map(|(text, prefix_width)| AnchorLiteral {
            text: text.clone(),
            prefix_width: *prefix_width,
        })
}

fn candidate_strategy_for(
    start_anchor: bool,
    anchor: Option<&AnchorLiteral>,
    patterns: &[Pattern],
    start_predicate: &StartPredicate,
) -> CandidateStrategy {
    if start_anchor {
        return CandidateStrategy::StartPredicateScan;
    }

    let Some(anchor) = anchor else {
        return CandidateStrategy::StartPredicateScan;
    };

    if anchor.prefix_width.is_some() {
        CandidateStrategy::FixedPrefixAnchor
    } else if should_use_variable_prefix_anchor(patterns, anchor, start_predicate) {
        CandidateStrategy::VariablePrefixLiteralAnchor
    } else {
        CandidateStrategy::StartPredicateScan
    }
}

fn should_use_variable_prefix_anchor(
    patterns: &[Pattern],
    anchor: &AnchorLiteral,
    start_predicate: &StartPredicate,
) -> bool {
    let Some((first, remaining)) = patterns.split_first() else {
        return false;
    };

    matches_simple_variable_prefix(first, start_predicate)
        && leading_literal_sequence(remaining).is_some_and(|(literal, _)| literal == anchor.text)
}

fn matches_simple_variable_prefix(pattern: &Pattern, start_predicate: &StartPredicate) -> bool {
    let count = pattern.count();
    let (min, max) = count.repetition_bounds();

    min > 0
        && max.is_none()
        && required_start_predicate(pattern)
            .as_ref()
            .is_some_and(|candidate| candidate == start_predicate)
}

fn first_required_start_predicate(patterns: &[Pattern]) -> Option<StartPredicate> {
    for pattern in patterns {
        if let Some(predicate) = required_start_predicate(pattern) {
            if pattern.count().repetition_bounds().0 > 0 {
                return Some(predicate);
            }
        }

        if pattern.count().repetition_bounds().0 > 0 {
            return None;
        }
    }

    None
}

fn required_start_predicate(pattern: &Pattern) -> Option<StartPredicate> {
    match pattern {
        Pattern::Literal(ch, _) => Some(StartPredicate::Literal(*ch)),
        Pattern::Digit(_) => Some(StartPredicate::Digit),
        Pattern::Alphanumeric(_) => Some(StartPredicate::Word),
        Pattern::Wildcard(_) => Some(StartPredicate::Wildcard),
        Pattern::CharGroup(group, _) => Some(StartPredicate::CharGroup(group.clone())),
        Pattern::CapturedGroup { patterns, .. } => first_required_start_predicate(patterns),
        Pattern::Alternation { alternatives, .. } => {
            let mut predicates = alternatives
                .iter()
                .map(|alternative| first_required_start_predicate(alternative))
                .collect::<Option<Vec<_>>>()?;
            let first = predicates.pop()?;
            predicates
                .into_iter()
                .all(|candidate| candidate == first)
                .then_some(first)
        }
        Pattern::Backreference(_) => None,
    }
}

fn leading_literal_sequence(patterns: &[Pattern]) -> Option<(String, usize)> {
    let mut literal = String::new();
    let mut consumed = 0;

    for pattern in patterns {
        let Some((ch, repetitions)) = literal_run_atom(pattern) else {
            break;
        };
        literal.extend(std::iter::repeat_n(ch, repetitions));
        consumed += 1;
    }

    (!literal.is_empty()).then_some((literal, consumed))
}

fn literal_run_atom(pattern: &Pattern) -> Option<(char, usize)> {
    match pattern {
        Pattern::Literal(ch, count) => count.fixed_repetitions().map(|count| (*ch, count)),
        _ => None,
    }
}

fn fixed_width(pattern: &Pattern) -> Option<usize> {
    let repetitions = pattern.count().fixed_repetitions()?;
    let single = match pattern {
        Pattern::Literal(_, _)
        | Pattern::Digit(_)
        | Pattern::Alphanumeric(_)
        | Pattern::Wildcard(_)
        | Pattern::CharGroup(_, _) => Some(1),
        Pattern::CapturedGroup { patterns, .. } => sequence_fixed_width(patterns),
        Pattern::Alternation { alternatives, .. } => {
            let mut widths = alternatives
                .iter()
                .map(|alternative| sequence_fixed_width(alternative));
            let first = widths.next()??;
            widths.all(|width| width == Some(first)).then_some(first)
        }
        Pattern::Backreference(_) => None,
    }?;

    Some(single * repetitions)
}

fn sequence_fixed_width(patterns: &[Pattern]) -> Option<usize> {
    let mut total = 0;
    for pattern in patterns {
        total += fixed_width(pattern)?;
    }
    Some(total)
}

fn visit_anchor_hits<T>(
    input: &str,
    scan_start: usize,
    literal: &str,
    mut visit: impl FnMut(usize) -> Option<T>,
) -> Option<T> {
    let mut search_from = scan_start;

    while let Some(anchor_start) = find_literal_from(input, search_from, literal) {
        if let Some(result) = visit(anchor_start) {
            return Some(result);
        }

        let Some(next) = next_char_boundary(input, anchor_start) else {
            break;
        };
        search_from = next;
    }

    None
}

fn visit_scan_candidates<T>(
    input: &str,
    scan_start: usize,
    upper_bound: usize,
    predicate: &StartPredicate,
    mut visit: impl FnMut(usize) -> Option<T>,
) -> Option<T> {
    let mut current = scan_start;
    while current <= upper_bound {
        if matches!(predicate, StartPredicate::Any) || predicate.matches_at(input, current) {
            if let Some(result) = visit(current) {
                return Some(result);
            }
        }
        let Some(next) = next_char_boundary(input, current) else {
            break;
        };
        current = next;
    }

    None
}

fn visit_bounded_backward_candidates<T>(
    input: &str,
    scan_start: usize,
    upper_bound: usize,
    predicate: &StartPredicate,
    mut visit: impl FnMut(usize) -> Option<T>,
) -> Option<T> {
    if upper_bound < scan_start || !predicate.matches_at(input, upper_bound) {
        return None;
    }

    let mut earliest = upper_bound;
    while let Some(previous) = previous_char_boundary(input, earliest) {
        if previous < scan_start || !predicate.matches_at(input, previous) {
            break;
        }
        earliest = previous;
    }

    let mut current = earliest;
    loop {
        if let Some(result) = visit(current) {
            return Some(result);
        }
        if current == upper_bound {
            break;
        }
        current = next_char_boundary(input, current).expect("candidate boundary should advance");
    }

    None
}

fn find_literal_from(input: &str, search_from: usize, literal: &str) -> Option<usize> {
    input
        .get(search_from..)?
        .find(literal)
        .map(|offset| search_from + offset)
}

fn matches_literal(input: &str, pos: usize, expected: char) -> bool {
    match_char(input, pos, |current| current == expected).is_some()
}

fn matches_digit(input: &str, pos: usize) -> bool {
    match_char(input, pos, |current| current.is_ascii_digit()).is_some()
}

fn matches_word(input: &str, pos: usize) -> bool {
    match_char(input, pos, |current| {
        current.is_ascii_alphanumeric() || current == '_'
    })
    .is_some()
}

fn matches_group(input: &str, pos: usize, group: &CharGroup) -> bool {
    match_char(input, pos, |current| group.matches(current)).is_some()
}

fn combine_widths(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    Some(left? + right?)
}

fn match_char(input: &str, pos: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
    let (matched, next) = current_char(input, pos)?;
    pred(matched).then_some(next)
}

fn current_char(input: &str, pos: usize) -> Option<(char, usize)> {
    let matched = input.get(pos..)?.chars().next()?;
    Some((matched, pos + matched.len_utf8()))
}

fn next_char_boundary(input: &str, pos: usize) -> Option<usize> {
    current_char(input, pos).map(|(_, next)| next)
}

fn previous_char_boundary(input: &str, pos: usize) -> Option<usize> {
    input.get(..pos)?.char_indices().last().map(|(idx, _)| idx)
}

fn advance_after_match(input: &str, start: usize, end: usize) -> usize {
    if end > start {
        end
    } else {
        next_char_boundary(input, start).unwrap_or(input.len())
    }
}
