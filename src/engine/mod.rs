mod automata;
mod backref;
mod classify;
mod literal;

use automata::AutomataSearch;
use backref::BackreferenceSearch;
use classify::{classify_regex, SearchStrategy};
use literal::LiteralSearch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegexMatch {
    pub start: usize,
    pub end: usize,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCandidate {
    Confirmed(usize),
    Candidate(usize),
}

pub struct CompiledRegex {
    plan: SearchPlan,
}

enum SearchPlan {
    Literal(LiteralSearch),
    Automata(AutomataSearch),
    Backreference(Box<BackreferenceSearch>),
}

impl CompiledRegex {
    fn new(regex: &str) -> Self {
        let plan = match classify_regex(regex) {
            SearchStrategy::Literal(spec) => SearchPlan::Literal(LiteralSearch::new(spec)),
            SearchStrategy::Automata => SearchPlan::Automata(AutomataSearch::new(regex)),
            SearchStrategy::Backreference => {
                SearchPlan::Backreference(Box::new(BackreferenceSearch::new(regex)))
            }
        };
        Self { plan }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn find_candidate_line(&self, input: &str, at: usize) -> Option<LineCandidate> {
        match &self.plan {
            SearchPlan::Literal(search) => search.find_candidate_line(input, at),
            SearchPlan::Automata(search) => search.find_candidate_line(input, at),
            SearchPlan::Backreference(search) => search.find_candidate_line(input, at),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn supports_candidate_lines(&self) -> bool {
        match &self.plan {
            SearchPlan::Literal(search) => search.supports_candidate_lines(),
            SearchPlan::Automata(search) => search.supports_candidate_lines(),
            SearchPlan::Backreference(search) => search.supports_candidate_lines(),
        }
    }
}

#[must_use]
pub fn compile_regex(regex: &str) -> CompiledRegex {
    CompiledRegex::new(regex)
}

#[must_use]
pub fn find_all_regex_spans_compiled(input_line: &str, regex: &CompiledRegex) -> Vec<RegexMatch> {
    match &regex.plan {
        SearchPlan::Literal(search) => search.find_all(input_line),
        SearchPlan::Automata(search) => search.find_all(input_line),
        SearchPlan::Backreference(search) => search.find_all(input_line),
    }
}
