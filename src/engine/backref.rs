use crate::pattern::{
    compile_backreference_regex, find_all_backreference_regex_spans_compiled,
    CompiledBackreferenceRegex,
};

use super::RegexMatch;

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
}
