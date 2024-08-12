use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(char, Count),
    Digit(Count),
    Alphanumeric(Count),
    Wildcard(Count),
    CharGroup(bool, String, Count),
    Alternation(Vec<Vec<Pattern>>),
    CapturedGroup(Vec<Pattern>),
    Backreference(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum Count {
    One,
    OneOrMore,
    ZeroOrOne,
}

pub fn parse(
    regex: &str,
) -> (
    Vec<Pattern>,
    bool, /* start_anchor */
    bool, /* end_anchor */
) {
    let start = regex.starts_with('^');
    let end = regex.ends_with('$');

    let mut patterns = vec![];
    let mut chars = regex.chars().peekable();
    if start {
        chars.next();
    }
    if end {
        chars.next_back();
    }

    loop {
        let Some(c) = chars.next() else {
            break;
        };
        let pattern = match c {
            '\\' => {
                let c = chars.next();
                if c.is_none() {
                    panic!("Expected character after '\\'");
                }
                let count = parse_count(&mut chars);
                match c.unwrap() {
                    'd' => Pattern::Digit(count),
                    'w' => Pattern::Alphanumeric(count),
                    '\\' => Pattern::Literal('\\', count),
                    backref if backref.is_ascii_digit() => {
                        let backref = backref.to_digit(10).unwrap();
                        Pattern::Backreference(backref as usize)
                    }
                    unknown => panic!("Unknown special character: {}", unknown),
                }
            }
            '[' => {
                let (is_positive, group) = parse_char_group(&mut chars);
                Pattern::CharGroup(is_positive, group, parse_count(&mut chars))
            }
            '.' => Pattern::Wildcard(parse_count(&mut chars)),
            '(' => {
                let mut patterns = parse_alternation(&mut chars);
                if patterns.len() == 1 {
                    Pattern::CapturedGroup(patterns.pop().unwrap())
                } else {
                    Pattern::Alternation(patterns)
                }
            }
            l => Pattern::Literal(l, parse_count(&mut chars)),
        };
        patterns.push(pattern);
    }

    (patterns, start, end)
}

fn parse_count(pattern: &mut Peekable<Chars>) -> Count {
    match pattern.next_if(|c| matches!(c, '+' | '?')) {
        Some('+') => Count::OneOrMore,
        Some('?') => Count::ZeroOrOne,
        _ => Count::One,
    }
}

fn parse_char_group(chars: &mut Peekable<Chars>) -> (bool, String) {
    let mut is_positive = true;
    let mut group = String::new();

    if chars.peek() == Some(&'^') {
        is_positive = false;
        chars.next();
    }

    while chars.peek() != Some(&']') {
        let c = chars.next();
        if c.is_none() {
            panic!("Expected ']' after group");
        }
        group.push(c.unwrap());
    }
    chars.next();

    (is_positive, group)
}

fn parse_alternation(chars: &mut Peekable<Chars>) -> Vec<Vec<Pattern>> {
    let mut alternation = Vec::new();
    while chars.peek() != Some(&')') {
        let mut c = chars.next();
        if c.is_none() {
            panic!("Expected ')' after alternation");
        }
        let mut regex = String::new();
        loop {
            regex.push(c.unwrap());
            if chars.peek() == Some(&')') || chars.peek() == Some(&'|') {
                break;
            }
            c = chars.next();
        }
        let (patterns, _, _) = parse(&regex);
        alternation.push(patterns);

        if chars.peek() == Some(&'|') {
            chars.next();
        }
    }
    chars.next();

    alternation
}
