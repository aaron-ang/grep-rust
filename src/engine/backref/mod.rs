mod ast;
mod parser;
mod runtime;

use ast::{compile_backreference_regex, find_all_backreference_regex_spans_compiled};
use runtime::CompiledBackreferenceRegex;

use super::{LineCandidate, RegexMatch};

pub(crate) struct BackreferenceSearch {
    regex: CompiledBackreferenceRegex,
}

impl BackreferenceSearch {
    pub(crate) fn new(pattern: &str) -> Self {
        Self {
            regex: compile_backreference_regex(pattern),
        }
    }

    pub(crate) fn find_all(&self, input: &str) -> Vec<RegexMatch> {
        find_all_backreference_regex_spans_compiled(input, &self.regex)
    }

    pub(crate) fn find_candidate_line(&self, _input: &str, _at: usize) -> Option<LineCandidate> {
        None
    }

    pub(crate) fn supports_candidate_lines(&self) -> bool {
        false
    }
}
