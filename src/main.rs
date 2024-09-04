use std::env;
use std::io;
use std::process;

mod r#match;
mod pattern;

use colored::Colorize;
use pattern::parse;
use r#match::match_substring;

fn match_regex(input_line: &str, regex: &str) -> Option<String> {
    let mut input_line = input_line.trim().chars().peekable();
    let (patterns, start, end) = parse(regex);
    let mut groups = vec![];
    let mut current_group = String::new();

    loop {
        let mut input_start = input_line.clone();
        if patterns
            .iter()
            .all(|p| match_substring(&mut input_start, p, &mut groups, &mut current_group))
        {
            if !end || input_start.peek().is_none() {
                return Some(current_group);
            } else {
                return None;
            }
        }
        if start {
            // first and only match failed
            return None;
        }
        if input_line.next().is_none() {
            return None;
        }
        current_group.clear();
        groups.clear();
    }
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    let first_arg = env::args().nth(1);
    if first_arg.is_none() || first_arg.unwrap() != "-E" {
        eprintln!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2);
    if pattern.is_none() {
        eprintln!("Expected second argument to be a pattern");
        process::exit(1);
    }

    let mut input_line = String::new();
    io::stdin().read_line(&mut input_line).unwrap();

    if let Some(group) = match_regex(&input_line, &pattern.unwrap()) {
        let i = input_line.find(&group).unwrap();
        let j = i + group.len();
        print!(
            "{}{}{}",
            input_line[..i].normal(),
            input_line[i..j].bright_red().bold(),
            input_line[j..].normal()
        );
        process::exit(0)
    } else {
        process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_backreferences() {
        assert_eq!(
            match_regex(
                "'cat and cat' is the same as 'cat and cat'",
                r"('(cat) and \2') is the same as \1"
            ),
            Some("'cat and cat' is the same as 'cat and cat'".to_string())
        );
        assert!(match_regex(
            "'cat and cat' is the same as 'cat and dog'",
            r"('(cat) and \2') is the same as \1"
        )
        .is_none());
        assert_eq!(
            match_regex(
                "grep 101 is doing grep 101 times, and again grep 101 times",
                r"((\w\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
            ),
            Some("grep 101 is doing grep 101 times, and again grep 101 times".to_string())
        );
        assert!(match_regex(
            "$?! 101 is doing $?! 101 times, and again $?! 101 times",
            r"((\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
        )
        .is_none());
        assert!(match_regex(
            "grep yes is doing grep yes times, and again grep yes times",
            r"((\w\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
        )
        .is_none());
        assert_eq!(
            match_regex(
                "abc-def is abc-def, not efg, abc, or def",
                r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
            ),
            Some("abc-def is abc-def, not efg, abc, or def".to_string())
        );
        assert!(match_regex(
            "efg-hij is efg-hij, not klm, efg, or hij",
            r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
        )
        .is_none());
        assert!(match_regex(
            "abc-def is abc-def, not xyz, abc, or def",
            r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
        )
        .is_none());
        assert_eq!(
            match_regex(
                "apple pie is made of apple and pie. love apple pie",
                r"^((\w+) (\w+)) is made of \2 and \3. love \1$"
            ),
            Some("apple pie is made of apple and pie. love apple pie".to_string())
        );
        assert!(match_regex(
            "pineapple pie is made of apple and pie. love apple pie",
            r"^((apple) (\w+)) is made of \2 and \3. love \1$"
        )
        .is_none());
        assert!(match_regex(
            "apple pie is made of apple and pie. love apple pies",
            r"^((\w+) (pie)) is made of \2 and \3. love \1$"
        )
        .is_none());
        assert_eq!(
            match_regex(
                "'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there",
                r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
            ),
            Some(
                "'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there".to_string()
            )
        );
        assert!(match_regex(
            "'howwdy heeey there' is made up of 'howwdy' and 'heeey'. howwdy heeey there",
            r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
        )
        .is_none());
        assert_eq!(
            match_regex(
                "cat and fish, cat with fish, cat and fish",
                r"((c.t|d.g) and (f..h|b..d)), \2 with \3, \1"
            ),
            Some("cat and fish, cat with fish, cat and fish".to_string())
        );
        assert!(match_regex(
            "bat and fish, bat with fish, bat and fish",
            r"((c.t|d.g) and (f..h|b..d)), \2 with \3, \1"
        )
        .is_none());
    }
}
