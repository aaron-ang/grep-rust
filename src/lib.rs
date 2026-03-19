mod engine;
#[doc(hidden)]
pub mod search_runner;
mod tests;

pub use engine::{
    compile_regex, find_all_regex_spans_compiled, CompiledRegex, LineCandidate, RegexMatch,
};
