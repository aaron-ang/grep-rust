#[derive(Debug)]
pub(crate) struct LiteralSpec {
    pub(crate) literals: Vec<String>,
    pub(crate) start_anchor: bool,
    pub(crate) end_anchor: bool,
}

#[derive(Debug)]
pub(crate) enum SearchStrategy {
    Literal(LiteralSpec),
    Automata,
    Backreference,
}

pub(crate) fn classify_regex(regex: &str) -> SearchStrategy {
    if has_backreference(regex) {
        return SearchStrategy::Backreference;
    }

    if let Some(spec) = extract_literal_spec(regex) {
        return SearchStrategy::Literal(spec);
    }

    SearchStrategy::Automata
}

fn has_backreference(regex: &str) -> bool {
    let mut chars = regex.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.next().is_some_and(|next| next.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn extract_literal_spec(regex: &str) -> Option<LiteralSpec> {
    let (regex, start_anchor) = strip_start_anchor(regex);
    let (regex, end_anchor) = strip_end_anchor(regex);

    if let Some(literal) = parse_literal(regex) {
        return Some(LiteralSpec {
            literals: vec![literal],
            start_anchor,
            end_anchor,
        });
    }

    parse_literal_alternation(regex).map(|literals| LiteralSpec {
        literals,
        start_anchor,
        end_anchor,
    })
}

fn strip_start_anchor(regex: &str) -> (&str, bool) {
    if let Some(stripped) = regex.strip_prefix('^') {
        (stripped, true)
    } else {
        (regex, false)
    }
}

fn strip_end_anchor(regex: &str) -> (&str, bool) {
    let mut chars = regex.char_indices().rev();
    match chars.next() {
        Some((idx, '$')) if !is_escaped(regex, idx) => (&regex[..idx], true),
        _ => (regex, false),
    }
}

fn parse_literal_alternation(regex: &str) -> Option<Vec<String>> {
    if !(regex.starts_with('(') && regex.ends_with(')')) {
        return None;
    }

    let inner = &regex[1..regex.len() - 1];
    let parts = split_top_level_alternation(inner)?;
    let literals: Option<Vec<_>> = parts.into_iter().map(parse_literal).collect();
    literals.filter(|literals| literals.len() > 1)
}

fn split_top_level_alternation(regex: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut in_class = false;
    let mut escaped = false;

    for (idx, ch) in regex.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => depth += 1,
            ')' if !in_class => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            '|' if !in_class && depth == 0 => {
                parts.push(&regex[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }

    if in_class || depth != 0 || escaped {
        return None;
    }

    if parts.is_empty() {
        return None;
    }

    parts.push(&regex[start..]);
    Some(parts)
}

fn parse_literal(regex: &str) -> Option<String> {
    let mut literal = String::new();
    let mut chars = regex.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let escaped = chars.next()?;
                if escaped.is_ascii_alphanumeric() {
                    return None;
                }
                literal.push(escaped);
            }
            '.' | '[' | '(' | ')' | '|' | '?' | '+' | '*' | '{' | '}' => return None,
            _ => literal.push(ch),
        }
    }

    (!literal.is_empty()).then_some(literal)
}

fn is_escaped(regex: &str, idx: usize) -> bool {
    let backslashes = regex[..idx]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count();
    backslashes % 2 == 1
}
