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
