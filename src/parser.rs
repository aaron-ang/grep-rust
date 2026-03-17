use std::{iter::Peekable, str::Chars};

use crate::pattern::{Alternation, Count, Group, Pattern};

pub struct Parser {
    group_idx: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self { group_idx: 0 }
    }

    pub fn parse(&mut self, chars: &mut Peekable<Chars>) -> Pattern {
        let c = chars.next().unwrap();
        match c {
            '\\' => Parser::parse_escape(chars),
            '[' => {
                let (negated, group) = Parser::parse_char_group(chars);
                Pattern::CharGroup(negated, group, Parser::parse_count(chars))
            }
            '.' => Pattern::Wildcard(Parser::parse_count(chars)),
            '(' => self.parse_group(chars),
            l => Pattern::Literal(l, Parser::parse_count(chars)),
        }
    }

    fn parse_escape(chars: &mut Peekable<Chars>) -> Pattern {
        let c = chars.next().expect("Expected character after '\\'");
        let count = Parser::parse_count(chars);
        match c {
            // Classes
            'd' => Pattern::Digit(count),
            'w' => Pattern::Alphanumeric(count),
            // Escaped backslash
            '\\' => Pattern::Literal('\\', count),
            // Backreferences
            c if c.is_ascii_digit() => Pattern::Backreference(c.to_digit(10).unwrap() as usize),
            // Unsupported characters
            unknown => panic!("Unknown special character: {unknown}"),
        }
    }

    fn parse_count(chars: &mut Peekable<Chars>) -> Count {
        match chars.peek() {
            Some('+') => {
                chars.next();
                Count::OneOrMore
            }
            Some('?') => {
                chars.next();
                Count::ZeroOrOne
            }
            Some('*') => {
                chars.next();
                Count::ZeroOrMore
            }
            Some('{') => Parser::parse_braced_count(chars),
            _ => Count::One,
        }
    }

    fn parse_braced_count(chars: &mut Peekable<Chars>) -> Count {
        chars.next();

        let mut count = String::new();
        loop {
            match chars.next() {
                Some('}') if !count.is_empty() => {
                    return Count::Exact(
                        count.parse().expect("exact quantifier should be a number"),
                    );
                }
                Some(',') if !count.is_empty() => match chars.next() {
                    Some('}') => {
                        return Count::AtLeast(
                            count
                                .parse()
                                .expect("lower bound quantifier should be a number"),
                        );
                    }
                    Some(c) if c.is_ascii_digit() => {
                        let mut upper = String::from(c);
                        loop {
                            match chars.next() {
                                Some('}') => {
                                    let lower = count
                                        .parse()
                                        .expect("range lower bound quantifier should be a number");
                                    let upper = upper
                                        .parse()
                                        .expect("range upper bound quantifier should be a number");
                                    if lower > upper {
                                        panic!("Expected '{{n,m}}' quantifier with n <= m");
                                    }
                                    return Count::Range(lower, upper);
                                }
                                Some(c) if c.is_ascii_digit() => upper.push(c),
                                _ => panic!("Expected '{{n,m}}' quantifier"),
                            }
                        }
                    }
                    _ => panic!("Expected '{{n,}}' or '{{n,m}}' quantifier"),
                },
                Some(c) if c.is_ascii_digit() => count.push(c),
                _ => panic!("Expected '{{n}}', '{{n,}}' or '{{n,m}}' quantifier"),
            }
        }
    }

    fn parse_char_group(chars: &mut Peekable<Chars>) -> (bool, String) {
        let negated = chars.peek() == Some(&'^');
        if negated {
            chars.next();
        }
        let mut group = String::new();
        loop {
            match chars.next() {
                None => panic!("Expected ']' after group"),
                Some(']') => break,
                Some(c) if c.is_ascii_alphanumeric() => group.push(c),
                Some(_) => panic!("Expected alphanumeric character in group"),
            }
        }
        (negated, group)
    }

    fn parse_group(&mut self, chars: &mut Peekable<Chars>) -> Pattern {
        let (idx, mut patterns) = self.parse_alternation(chars);
        let count = Parser::parse_count(chars);
        if patterns.len() == 1 {
            Pattern::CapturedGroup(Group {
                idx,
                patterns: patterns.pop().unwrap(),
                count,
            })
        } else {
            Pattern::Alternation(Alternation {
                idx,
                alternatives: patterns,
                count,
            })
        }
    }

    fn parse_alternation(&mut self, chars: &mut Peekable<Chars>) -> (usize, Vec<Vec<Pattern>>) {
        let mut alternation = Vec::new();
        let mut group_chars = String::new();
        let mut num_open_parens = 0;
        let idx = self.group_idx;
        self.group_idx += 1;
        loop {
            match chars.next() {
                None => panic!("Expected ')' after alternation"),
                Some(c) => match c {
                    '(' => {
                        num_open_parens += 1;
                        group_chars.push('(');
                    }
                    ')' => {
                        if num_open_parens == 0 {
                            alternation
                                .push(self.read_group_items(&mut group_chars.chars().peekable()));
                            break;
                        } else {
                            num_open_parens -= 1;
                            group_chars.push(')');
                        }
                    }
                    '|' if num_open_parens == 0 => {
                        alternation
                            .push(self.read_group_items(&mut group_chars.chars().peekable()));
                        group_chars.clear();
                    }
                    _ => group_chars.push(c),
                },
            }
        }
        (idx, alternation)
    }

    fn read_group_items(&mut self, pattern: &mut Peekable<Chars>) -> Vec<Pattern> {
        let mut items = Vec::new();
        while pattern.peek().is_some() {
            items.push(self.parse(pattern))
        }
        items
    }
}
