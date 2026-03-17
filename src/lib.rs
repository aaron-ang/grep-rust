mod parser;
mod pattern;
mod tests;

pub use pattern::{compile_regex, find_all_regex_spans_compiled, CompiledRegex, RegexMatch};
