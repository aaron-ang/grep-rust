use std::{iter::Peekable, str::Chars};

use crate::pattern::{Alternation, Count, Group, Pattern};

pub struct Parser {
    group_idx: usize,
    nesting: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            group_idx: 0,
            nesting: 0,
        }
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
            'd' => Pattern::Digit(count),
            'w' => Pattern::Alphanumeric(count),
            '\\' => Pattern::Literal('\\', count),
            backref if backref.is_ascii_digit() => {
                let backref = backref.to_digit(10).unwrap();
                Pattern::Backreference(backref as usize)
            }
            unknown => panic!("Unknown special character: {unknown}"),
        }
    }

    fn parse_count(pattern: &mut Peekable<Chars>) -> Count {
        match pattern.next_if(|c| matches!(c, '+' | '?')) {
            Some('+') => Count::OneOrMore,
            Some('?') => Count::ZeroOrOne,
            _ => Count::One,
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
        if patterns.len() == 1 {
            Pattern::CapturedGroup(Group {
                idx,
                patterns: patterns.pop().unwrap(),
                count: Parser::parse_count(chars),
            })
        } else {
            Pattern::Alternation(Alternation {
                idx,
                alternatives: patterns,
                count: Parser::parse_count(chars),
            })
        }
    }

    fn parse_alternation(&mut self, chars: &mut Peekable<Chars>) -> (usize, Vec<Vec<Pattern>>) {
        let mut alternation = vec![];
        let mut group_chars = String::new();
        let mut num_open_parens = 0;
        self.nesting += 1;
        self.group_idx += 1;
        let idx = self.group_idx;
        loop {
            match chars.next() {
                None => panic!("Expected ')' after alternation"),
                Some('(') => {
                    num_open_parens += 1;
                    group_chars.push('(');
                }
                Some(')') => {
                    if num_open_parens == 0 {
                        alternation
                            .push(self.read_group_items(&mut group_chars.chars().peekable()));
                        break;
                    } else {
                        num_open_parens -= 1;
                        group_chars.push(')');
                    }
                }
                Some('|') => {
                    if num_open_parens == 0 {
                        alternation
                            .push(self.read_group_items(&mut group_chars.chars().peekable()));
                        group_chars.clear();
                    } else {
                        group_chars.push('|');
                    }
                }
                Some(c) => group_chars.push(c),
            }
        }
        self.nesting -= 1;
        (idx, alternation)
    }

    fn read_group_items(&mut self, pattern: &mut Peekable<Chars>) -> Vec<Pattern> {
        let mut items = vec![];
        while pattern.peek().is_some() {
            items.push(self.parse(pattern))
        }
        items
    }
}
