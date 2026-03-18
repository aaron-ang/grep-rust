mod engine;
mod parser;
mod pattern;
mod tests;

pub use engine::{compile_regex, find_all_regex_spans_compiled, CompiledRegex, RegexMatch};
