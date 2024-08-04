use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(char),
    Digit,
    Alphanumeric,
    Group(bool, String),
    OneOrMore(Box<Pattern>),
    ZeroOrOne(Box<Pattern>),
    Wildcard,
    Alternation(Vec<Vec<Pattern>>),
}

pub fn get_patterns(regex: &str) -> Vec<Pattern> {
    let mut patterns: Vec<Pattern> = Vec::new();
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
                    backref if backref.is_ascii_digit() => {
                        let mut backref = backref.to_digit(10).unwrap();
                        while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                            let c = chars.next();
                            backref *= 10;
                            backref += c.unwrap().to_digit(10).unwrap();
                        }
                        find_group(&patterns, backref as usize)
                    }
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
                let pattern = patterns.pop().expect("Expected pattern before '?'");
                Pattern::ZeroOrOne(Box::new(pattern))
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

fn find_group(patterns: &[Pattern], backref: usize) -> Pattern {
    let group = patterns
        .iter()
        .filter(|p| matches!(p, Pattern::Alternation(_)))
        .nth(backref - 1);
    if let Some(group) = group {
        return group.clone();
    }
    panic!("Backreference not found");
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
