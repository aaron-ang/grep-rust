use regex_automata::meta::Regex;

use super::RegexMatch;

pub(crate) struct AutomataSearch {
    regex: Regex,
}

impl AutomataSearch {
    pub(crate) fn new(pattern: &str) -> Self {
        let regex = Regex::new(pattern)
            .unwrap_or_else(|err| panic!("invalid regex for automata engine: {err}"));
        Self { regex }
    }

    pub(crate) fn find_all(&self, input: &str) -> Vec<RegexMatch> {
        self.regex
            .find_iter(input)
            .map(|matched| RegexMatch {
                start: matched.start(),
                end: matched.end(),
            })
            .collect()
    }
}
