use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};

use super::{classify::LiteralSpec, LineCandidate, RegexMatch};

pub(crate) struct LiteralSearch {
    literals: Vec<String>,
    automaton: AhoCorasick,
    start_anchor: bool,
    end_anchor: bool,
}

impl LiteralSearch {
    pub(crate) fn new(spec: LiteralSpec) -> Self {
        let automaton = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostFirst)
            .build(&spec.literals)
            .unwrap_or_else(|err| panic!("failed to build aho-corasick automaton: {err}"));
        Self {
            literals: spec.literals,
            automaton,
            start_anchor: spec.start_anchor,
            end_anchor: spec.end_anchor,
        }
    }

    pub(crate) fn find_all(&self, input: &str) -> Vec<RegexMatch> {
        if self.start_anchor && self.end_anchor {
            return self
                .literals
                .iter()
                .find(|literal| input == literal.as_str())
                .map(|literal| {
                    vec![RegexMatch {
                        start: 0,
                        end: literal.len(),
                    }]
                })
                .unwrap_or_default();
        }

        if self.start_anchor {
            return self
                .literals
                .iter()
                .find(|literal| input.starts_with(literal.as_str()))
                .map(|literal| {
                    vec![RegexMatch {
                        start: 0,
                        end: literal.len(),
                    }]
                })
                .unwrap_or_default();
        }

        if self.end_anchor {
            return suffix_match(input, &self.literals)
                .map(|(start, end)| vec![RegexMatch { start, end }])
                .unwrap_or_default();
        }

        self.automaton
            .find_iter(input)
            .map(|matched| RegexMatch {
                start: matched.start(),
                end: matched.end(),
            })
            .collect()
    }

    pub(crate) fn find_candidate_line(&self, input: &str, at: usize) -> Option<LineCandidate> {
        let offset = self.automaton.find(input.get(at..)?)?.start() + at;
        if self.start_anchor || self.end_anchor {
            Some(LineCandidate::Candidate(offset))
        } else {
            Some(LineCandidate::Confirmed(offset))
        }
    }

    pub(crate) fn supports_candidate_lines(&self) -> bool {
        true
    }
}

fn suffix_match(input: &str, literals: &[String]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;

    for (index, literal) in literals.iter().enumerate() {
        if input.ends_with(literal) {
            let start = input.len() - literal.len();
            let candidate = (start, index);
            if best.is_none_or(|current| candidate < current) {
                best = Some(candidate);
            }
        }
    }

    best.map(|(start, index)| (start, start + literals[index].len()))
}
