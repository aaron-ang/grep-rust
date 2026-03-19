use regex_automata::meta::Regex;

use super::{LineCandidate, RegexMatch};

pub(crate) struct AutomataSearch {
    regex: Regex,
    literal_prefix: Option<String>,
}

impl AutomataSearch {
    pub(crate) fn new(pattern: &str) -> Self {
        let regex = Regex::new(pattern)
            .unwrap_or_else(|err| panic!("invalid regex for automata engine: {err}"));
        Self {
            regex,
            literal_prefix: extract_literal_prefix(pattern),
        }
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

    pub(crate) fn find_candidate_line(&self, input: &str, at: usize) -> Option<LineCandidate> {
        let prefix = self.literal_prefix.as_deref()?;
        let offset = input.get(at..)?.find(prefix)? + at;
        Some(LineCandidate::Candidate(offset))
    }

    pub(crate) fn supports_candidate_lines(&self) -> bool {
        self.literal_prefix.is_some()
    }
}

fn extract_literal_prefix(pattern: &str) -> Option<String> {
    let mut literal = String::new();
    let mut chars = pattern
        .strip_prefix('^')
        .unwrap_or(pattern)
        .chars()
        .peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let escaped = chars.next()?;
                if escaped.is_ascii_alphanumeric() {
                    break;
                }
                literal.push(escaped);
            }
            '.' | '[' | '(' | ')' | '|' | '?' | '+' | '*' | '{' => break,
            '$' if chars.peek().is_none() => break,
            _ => literal.push(ch),
        }
    }

    (!literal.is_empty()).then_some(literal)
}
