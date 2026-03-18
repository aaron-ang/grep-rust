#[cfg(test)]
use super::*;
#[cfg(test)]
use crate::engine::EngineKind;

#[cfg(test)]
fn find_all_regex(input_line: &str, regex: &str) -> Vec<String> {
    let compiled = compile_regex(regex);
    find_all_regex_spans_compiled(input_line, &compiled)
        .into_iter()
        .map(|matched| input_line[matched.start..matched.end].to_string())
        .collect()
}

#[cfg(test)]
fn find_all_regex_spans(input_line: &str, regex: &str) -> Vec<RegexMatch> {
    let compiled = compile_regex(regex);
    find_all_regex_spans_compiled(input_line, &compiled)
}

#[cfg(test)]
fn first_match(input_line: &str, regex: &str) -> Option<String> {
    find_all_regex(input_line, regex).into_iter().next()
}

#[test]
fn alphanumeric() {
    assert_eq!(first_match("÷%+_#×+", r"\w"), Some("_".to_string()));
}

#[test]
fn alternation() {
    assert_eq!(
        first_match(
            "I see 1 cat, 2 dogs and 3 cows",
            r"^I see (\d (cat|dog|cow)s?(, | and )?)+$"
        ),
        Some("I see 1 cat, 2 dogs and 3 cows".to_string())
    );
}

#[test]
fn nested_backreferences_literal() {
    assert_eq!(
        first_match(
            "'cat and cat' is the same as 'cat and cat'",
            r"('(cat) and \2') is the same as \1"
        ),
        Some("'cat and cat' is the same as 'cat and cat'".to_string())
    );
    assert!(first_match(
        "'cat and cat' is the same as 'cat and dog'",
        r"('(cat) and \2') is the same as \1"
    )
    .is_none());
}

#[test]
fn nested_backreferences_digit_alphanumeric() {
    assert_eq!(
        first_match(
            "grep 101 is doing grep 101 times, and again grep 101 times",
            r"((\w\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
        ),
        Some("grep 101 is doing grep 101 times, and again grep 101 times".to_string())
    );
    assert!(first_match(
        "$?! 101 is doing $?! 101 times, and again $?! 101 times",
        r"((\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
    )
    .is_none());
    assert!(first_match(
        "grep yes is doing grep yes times, and again grep yes times",
        r"((\w\w\w\w) (\d\d\d)) is doing \2 \3 times, and again \1 times"
    )
    .is_none());
}

#[test]
fn nested_backreferences_grouping() {
    assert_eq!(
        first_match(
            "abc-def is abc-def, not efg, abc, or def",
            r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
        ),
        Some("abc-def is abc-def, not efg, abc, or def".to_string())
    );
    assert!(first_match(
        "efg-hij is efg-hij, not klm, efg, or hij",
        r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
    )
    .is_none());
    assert!(first_match(
        "abc-def is abc-def, not xyz, abc, or def",
        r"(([abc]+)-([def]+)) is \1, not ([^xyz]+), \2, or \3"
    )
    .is_none());
}

#[test]
fn nested_backreferences_anchor() {
    assert_eq!(
        first_match(
            "apple pie is made of apple and pie. love apple pie",
            r"^((\w+) (\w+)) is made of \2 and \3. love \1$"
        ),
        Some("apple pie is made of apple and pie. love apple pie".to_string())
    );
    assert!(first_match(
        "pineapple pie is made of apple and pie. love apple pie",
        r"^((apple) (\w+)) is made of \2 and \3. love \1$"
    )
    .is_none());
    assert!(first_match(
        "apple pie is made of apple and pie. love apple pies",
        r"^((\w+) (pie)) is made of \2 and \3. love \1$"
    )
    .is_none());
}

#[test]
fn nested_backreferences_quantifier() {
    assert_eq!(
        first_match(
            "'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there",
            r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
        ),
        Some("'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there".to_string())
    );
    assert!(first_match(
        "'howwdy heeey there' is made up of 'howwdy' and 'heeey'. howwdy heeey there",
        r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
    )
    .is_none());
}

#[test]
fn nested_backreferences_wildcard_alternation() {
    assert_eq!(
        first_match(
            "cat and fish, cat with fish, cat and fish",
            r"((c.t|d.g) and (f..h|b..d)), \2 with \3, \1"
        ),
        Some("cat and fish, cat with fish, cat and fish".to_string())
    );
    assert!(first_match(
        "bat and fish, bat with fish, bat and fish",
        r"((c.t|d.g) and (f..h|b..d)), \2 with \3, \1"
    )
    .is_none());
}

#[test]
fn literal_character_matching() {
    assert_eq!(first_match("dog", "d"), Some("d".to_string()));
    assert!(first_match("dog", "f").is_none());
}

#[test]
fn wildcard_matching() {
    assert_eq!(first_match("cat", "c.t"), Some("cat".to_string()));
    assert!(first_match("car", "c.t").is_none());
    assert_eq!(
        first_match("goøö0Ogol", "g.+gol"),
        Some("goøö0Ogol".to_string())
    );
    assert!(first_match("gol", "g.+gol").is_none());
}

#[test]
fn multiple_only_matches() {
    assert_eq!(
        find_all_regex("The king had 10 children", r"\d"),
        vec!["1".to_string(), "0".to_string()]
    );
    assert_eq!(
        find_all_regex("The king had 10 children", r"\d\d"),
        vec!["10".to_string()]
    );
    assert_eq!(
        find_all_regex("jekyll and hyde", "(jekyll|hyde)"),
        vec!["jekyll".to_string(), "hyde".to_string()]
    );
    assert!(find_all_regex("no match here", r"\d").is_empty());
}

#[test]
fn digit_matching() {
    assert_eq!(first_match("123", r"\d"), Some("1".to_string()));
    assert!(first_match("apple", r"\d").is_none());
    assert_eq!(first_match("abc_0_xyz", r"\d"), Some("0".to_string()));
}

#[test]
fn word_character_matching() {
    assert_eq!(first_match("grape", r"\w"), Some("g".to_string()));
    assert_eq!(first_match("RASPBERRY", r"\w"), Some("R".to_string()));
    assert_eq!(first_match("303", r"\w"), Some("3".to_string()));
    assert_eq!(first_match("-#+_-+×", r"\w"), Some("_".to_string()));
    assert!(first_match("%#=÷-+", r"\w").is_none());
}

#[test]
fn positive_character_groups() {
    assert_eq!(first_match("b", "[strawberry]"), Some("b".to_string()));
    assert_eq!(first_match("bcd", "[strawberry]"), Some("b".to_string()));
    assert!(first_match("pineapple", "[bcdfghjkm]").is_none());
    assert!(first_match("[]", "[mango]").is_none());
}

#[test]
fn negative_character_groups() {
    assert_eq!(first_match("apple", "[^xyz]"), Some("a".to_string()));
    assert_eq!(first_match("apple", "[^abc]"), Some("p".to_string()));
    assert!(first_match("banana", "[^anb]").is_none());
    assert_eq!(first_match("orange", "[^opq]"), Some("r".to_string()));
}

#[test]
fn combining_character_classes() {
    assert_eq!(
        first_match("sally has 3 apples", r"\d apple"),
        Some("3 apple".to_string())
    );
    assert!(first_match("sally has 1 orange", r"\d apple").is_none());
    assert_eq!(
        first_match("sally has 124 apples", r"\d\d\d apples"),
        Some("124 apples".to_string())
    );
    assert!(first_match("sally has 12 apples", r"\d\\d\\d apples").is_none());
    assert_eq!(
        first_match("sally has 3 dogs", r"\d \w\w\ws"),
        Some("3 dogs".to_string())
    );
    assert_eq!(
        first_match("sally has 4 dogs", r"\d \w\w\ws"),
        Some("4 dogs".to_string())
    );
    assert!(first_match("sally has 1 dog", r"\d \w\w\ws").is_none());
}

#[test]
fn anchors() {
    // Start anchor
    assert_eq!(
        first_match("mango_apple", "^mango"),
        Some("mango".to_string())
    );
    assert!(first_match("apple_mango", "^mango").is_none());

    // End anchor
    assert_eq!(
        first_match("strawberry_blueberry", "blueberry$"),
        Some("blueberry".to_string())
    );
    assert!(first_match("blueberry_strawberry", "blueberry$").is_none());

    // Both anchors
    assert_eq!(first_match("pear", "^pear$"), Some("pear".to_string()));
    assert!(first_match("pear_pear", "^pear$").is_none());
}

#[test]
fn quantifiers_zero_or_one() {
    assert_eq!(first_match("cat", "ca?t"), Some("cat".to_string()));
    assert_eq!(first_match("act", "ca?t"), Some("ct".to_string()));
    assert_eq!(first_match("cat", "ca?a?t"), Some("cat".to_string()));
    assert!(first_match("dog", "ca?t").is_none());
    assert!(first_match("cag", "ca?t").is_none());
}

#[test]
fn quantifiers_one_or_more() {
    assert_eq!(first_match("cat", "ca+t"), Some("cat".to_string()));
    assert_eq!(first_match("caaats", "ca+at"), Some("caaat".to_string()));
    assert!(first_match("act", "ca+t").is_none());
    assert!(first_match("ca", "ca+t").is_none());
    assert_eq!(
        first_match("abc_123_xyz", r"^abc_\d+_xyz$"),
        Some("abc_123_xyz".to_string())
    );
    assert!(first_match("abc_rst_xyz", r"^abc_\d+_xyz$").is_none());
}

#[test]
fn quantifiers_zero_or_more() {
    assert_eq!(first_match("apple", "apple*"), Some("apple".to_string()));
    assert_eq!(first_match("appl", "apple*"), Some("appl".to_string()));
    assert_eq!(
        first_match("horse54tiger", r"horse\d*tiger"),
        Some("horse54tiger".to_string())
    );
    assert_eq!(
        first_match("peas,_fresh", r"peas,\w*"),
        Some("peas,_fresh".to_string())
    );
    assert_eq!(
        first_match("tigerhorsehorsetiger", "(horse|tiger)*"),
        Some("tigerhorsehorsetiger".to_string())
    );
    assert_eq!(
        first_match("LOG INFO 21 apple", r"^LOG [FION]* \d+ (apple|peas)$"),
        Some("LOG INFO 21 apple".to_string())
    );
    assert!(first_match("LOG info 85 apple", r"^LOG [FION]* \d+ (apple|peas)$").is_none());
}

#[test]
fn quantifiers_exact_count() {
    assert_eq!(first_match("caaat", r"ca{3}t"), Some("caaat".to_string()));
    assert!(first_match("caat", r"ca{3}t").is_none());
    assert!(first_match("caaaat", r"ca{3}t").is_none());
    assert_eq!(first_match("d42g", r"d\d{2}g"), Some("d42g".to_string()));
    assert!(first_match("d1g", r"d\d{2}g").is_none());
    assert!(first_match("d123g", r"d\d{2}g").is_none());
    assert_eq!(
        first_match("czyxzw", r"c[xyz]{4}w"),
        Some("czyxzw".to_string())
    );
    assert!(first_match("cxyzw", r"c[xyz]{4}w").is_none());
}

#[test]
fn quantifiers_at_least_count() {
    assert_eq!(first_match("caat", r"ca{2,}t"), Some("caat".to_string()));
    assert_eq!(
        first_match("caaaaat", r"ca{2,}t"),
        Some("caaaaat".to_string())
    );
    assert!(first_match("cat", r"ca{2,}t").is_none());
    assert_eq!(
        first_match("x9999y", r"x\d{3,}y"),
        Some("x9999y".to_string())
    );
    assert!(first_match("x42y", r"x\d{3,}y").is_none());
    assert_eq!(
        first_match("baeiour", r"b[aeiou]{2,}r"),
        Some("baeiour".to_string())
    );
    assert!(first_match("bar", r"b[aeiou]{2,}r").is_none());
}

#[test]
fn quantifiers_bounded_range() {
    assert_eq!(first_match("caat", r"ca{2,4}t"), Some("caat".to_string()));
    assert_eq!(first_match("caaat", r"ca{2,4}t"), Some("caaat".to_string()));
    assert_eq!(
        first_match("caaaat", r"ca{2,4}t"),
        Some("caaaat".to_string())
    );
    assert!(first_match("caaaaat", r"ca{2,4}t").is_none());
    assert_eq!(
        first_match("n123m", r"n\d{1,3}m"),
        Some("n123m".to_string())
    );
    assert!(first_match("n1234m", r"n\d{1,3}m").is_none());
    assert_eq!(
        first_match("pzzzq", r"p[xyz]{2,3}q"),
        Some("pzzzq".to_string())
    );
    assert!(first_match("pxq", r"p[xyz]{2,3}q").is_none());
    assert!(first_match("pxyzyq", r"p[xyz]{2,3}q").is_none());
}

#[test]
fn alternation_basic() {
    assert_eq!(
        first_match("a cat", "a (cat|dog)"),
        Some("a cat".to_string())
    );
    assert!(first_match("a cog", "a (cat|dog)").is_none());
    assert_eq!(
        first_match("I see 1 cat", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 1 cat".to_string())
    );
    assert_eq!(
        first_match("I see 42 dogs", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 42 dogs".to_string())
    );
    assert!(first_match("I see a cat", r"^I see \d+ (cat|dog)s?$").is_none());
    assert!(first_match("I see 2 dog3", r"^I see \d+ (cat|dog)s?$").is_none());
}

#[test]
fn single_backreferences() {
    assert_eq!(
        first_match("cat and cat", "(cat) and \\1"),
        Some("cat and cat".to_string())
    );
    assert!(first_match("cat and dog", "(cat) and \\1").is_none());
    assert_eq!(
        first_match("cat and cat", r"(\w+) and \1"),
        Some("cat and cat".to_string())
    );
    assert!(first_match("cat and dog", r"(\w+) and \1").is_none());
    assert_eq!(
        first_match("cat is cat, not dog", r"^([act]+) is \1, not [^xyz]+$"),
        Some("cat is cat, not dog".to_string())
    );
    assert!(first_match("cat is c@t, not d0g", r"^([act]+) is \1, not [^xyz]+$").is_none());
}

#[test]
fn multiple_backreferences() {
    assert_eq!(
        first_match(
            "3 red squares and 3 red circles",
            r"(\d+) (\w+) squares and \1 \2 circles"
        ),
        Some("3 red squares and 3 red circles".to_string())
    );
    assert!(first_match(
        "3 red squares and 4 red circles",
        r"(\d+) (\w+) squares and \1 \2 circles"
    )
    .is_none());
    assert_eq!(
        first_match(
            "grep 101 is doing grep 101 times",
            r"(\w\w\w\w) (\d\d\d) is doing \1 \2 times"
        ),
        Some("grep 101 is doing grep 101 times".to_string())
    );
    assert!(first_match(
        "$?! 101 is doing $?! 101 times",
        r"(\w\w\w) (\d\d\d) is doing \1 \2 times"
    )
    .is_none());
    assert!(first_match(
        "grep yes is doing grep yes times",
        r"(\w\w\w\w) (\d\d\d) is doing \1 \2 times"
    )
    .is_none());
    assert_eq!(
        first_match(
            "abc-def is abc-def, not efg",
            r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
        ),
        Some("abc-def is abc-def, not efg".to_string())
    );
    assert!(first_match(
        "efg-hij is efg-hij, not efg",
        r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
    )
    .is_none());
    assert!(first_match(
        "abc-def is abc-def, not xyz",
        r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
    )
    .is_none());
    assert_eq!(
        first_match("apple pie, apple and pie", r"^(\w+) (\w+), \1 and \2$"),
        Some("apple pie, apple and pie".to_string())
    );
    assert!(first_match(
        "pineapple pie, pineapple and pie",
        r"^(apple) (\w+), \1 and \2$"
    )
    .is_none());
    assert!(first_match("apple pie, apple and pies", r"^(\w+) (pie), \1 and \2$").is_none());
    assert_eq!(
        first_match(
            "howwdy hey there, howwdy hey",
            r"(how+dy) (he?y) there, \1 \2"
        ),
        Some("howwdy hey there, howwdy hey".to_string())
    );
    assert!(first_match(
        "hody hey there, howwdy hey",
        r"(how+dy) (he?y) there, \1 \2"
    )
    .is_none());
    assert!(first_match(
        "howwdy heeey there, howwdy heeey",
        r"(how+dy) (he?y) there, \1 \2"
    )
    .is_none());
    assert_eq!(
        first_match(
            "cat and fish, cat with fish",
            r"(c.t|d.g) and (f..h|b..d), \1 with \2"
        ),
        Some("cat and fish, cat with fish".to_string())
    );
    assert!(first_match(
        "bat and fish, cat with fish",
        r"(c.t|d.g) and (f..h|b..d), \1 with \2"
    )
    .is_none());
}

#[test]
fn match_spans_report_positions() {
    assert_eq!(
        find_all_regex_spans("jekyll and hyde", "(jekyll|hyde)"),
        vec![
            RegexMatch { start: 0, end: 6 },
            RegexMatch { start: 11, end: 15 },
        ]
    );
    assert_eq!(
        find_all_regex_spans("goøö0Ogol", "g.+gol"),
        vec![RegexMatch {
            start: 0,
            end: "goøö0Ogol".len(),
        }]
    );
}

#[test]
fn compiled_regex_can_be_reused() {
    let compiled = compile_regex(r"\d");
    assert_eq!(compiled.engine_kind(), EngineKind::Automata);
    assert_eq!(
        find_all_regex_spans_compiled("The king had 10 children", &compiled),
        vec![
            RegexMatch { start: 13, end: 14 },
            RegexMatch { start: 14, end: 15 }
        ]
    );
    assert_eq!(
        find_all_regex_spans_compiled("No digits here", &compiled),
        Vec::new()
    );
}

#[test]
fn routes_patterns_to_the_expected_engine() {
    assert_eq!(compile_regex("hello").engine_kind(), EngineKind::Literal);
    assert_eq!(
        compile_regex(r"^(hello|world)$").engine_kind(),
        EngineKind::Literal
    );
    assert_eq!(
        compile_regex(r"hello\d+").engine_kind(),
        EngineKind::Automata
    );
    assert_eq!(
        compile_regex(r"(\w+) and \1").engine_kind(),
        EngineKind::Backreference
    );
}

#[test]
fn complex_quantifier_combinations() {
    // Test the specific case that was failing before
    assert_eq!(
        first_match("pandadogdogpanda", "(dog|panda)*"),
        Some("pandadogdogpanda".to_string())
    );

    // Test digit patterns with quantifiers
    assert_eq!(
        first_match("I see 1 cat", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 1 cat".to_string())
    );
    assert!(first_match("I see a cat", r"^I see \d+ (cat|dog)s?$").is_none());

    // Test character groups with quantifiers
    assert_eq!(
        first_match("LOG INFO 21 apple", r"^LOG [FION]* \d+ (apple|peas)$"),
        Some("LOG INFO 21 apple".to_string())
    );
    assert!(first_match("LOG info 85 apple", r"^LOG [FION]* \d+ (apple|peas)$").is_none());
}

#[test]
fn grouped_regexes_without_backreferences_match_normally() {
    let grouped = compile_regex(r"(cat|dog)+");
    assert_eq!(
        find_all_regex_spans_compiled("xxcatdogyy", &grouped),
        vec![RegexMatch { start: 2, end: 8 }]
    );
}
