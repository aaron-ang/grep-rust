use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(char),
    Digit,
    Alphanumeric,
    Group(bool, String),
    OneOrMore(Box<Pattern>),
    ZeroOrMore(Box<Pattern>),
    Wildcard,
    Alternation(Vec<Vec<Pattern>>),
}

pub fn get_patterns(regex: &str) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    let mut chars = regex.chars().peekable();

    loop {
        let c = chars.next();
        if c.is_none() {
            break;
        }
        let pattern = match c.unwrap() {
            '\\' => {
                let c = chars.next();
                if c.is_none() {
                    panic!("Expected character after '\\'");
                }
                match c.unwrap() {
                    'd' => Pattern::Digit,
                    'w' => Pattern::Alphanumeric,
                    '\\' => Pattern::Literal('\\'),
                    unknown => panic!("Unknown special character: {}", unknown),
                }
            }
            '[' => {
                let (is_positive, group) = get_group_pattern(&mut chars);
                Pattern::Group(is_positive, group)
            }
            '+' => {
                let pattern = patterns.pop().expect("Expected pattern before '+'");
                Pattern::OneOrMore(Box::new(pattern))
            }
            '?' => {
                let pattern = patterns.pop().expect("Expected pattern before '*'");
                Pattern::ZeroOrMore(Box::new(pattern))
            }
            '.' => Pattern::Wildcard,
            '(' => {
                let patterns = get_alternation_pattern(&mut chars);
                Pattern::Alternation(patterns)
            }
            l => Pattern::Literal(l),
        };
        patterns.push(pattern);
    }

    patterns
}

fn get_group_pattern(chars: &mut Peekable<Chars>) -> (bool, String) {
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

fn get_alternation_pattern(chars: &mut Peekable<Chars>) -> Vec<Vec<Pattern>> {
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
        let patterns = get_patterns(&regex);
        alternation.push(patterns);

        if chars.peek() == Some(&'|') {
            chars.next();
        }
    }
    chars.next();

    alternation
}
