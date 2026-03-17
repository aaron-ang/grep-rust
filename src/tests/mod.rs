#[cfg(test)]
use super::*;

#[test]
fn alphanumeric() {
    assert_eq!(match_regex("÷%+_#×+", r"\w"), Some("_".to_string()));
}

#[test]
fn alternation() {
    assert_eq!(
        match_regex(
            "I see 1 cat, 2 dogs and 3 cows",
            r"^I see (\d (cat|dog|cow)s?(, | and )?)+$"
        ),
        Some("I see 1 cat, 2 dogs and 3 cows".to_string())
    );
}

#[test]
fn nested_backreferences_literal() {
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
}

#[test]
fn nested_backreferences_digit_alphanumeric() {
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
}

#[test]
fn nested_backreferences_grouping() {
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
}

#[test]
fn nested_backreferences_anchor() {
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
}

#[test]
fn nested_backreferences_quantifier() {
    assert_eq!(
        match_regex(
            "'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there",
            r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
        ),
        Some("'howwdy hey there' is made up of 'howwdy' and 'hey'. howwdy hey there".to_string())
    );
    assert!(match_regex(
        "'howwdy heeey there' is made up of 'howwdy' and 'heeey'. howwdy heeey there",
        r"'((how+dy) (he?y) there)' is made up of '\2' and '\3'. \1"
    )
    .is_none());
}

#[test]
fn nested_backreferences_wildcard_alternation() {
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

#[test]
fn literal_character_matching() {
    assert_eq!(match_regex("dog", "d"), Some("d".to_string()));
    assert!(match_regex("dog", "f").is_none());
}

#[test]
fn wildcard_matching() {
    assert_eq!(match_regex("cat", "c.t"), Some("cat".to_string()));
    assert!(match_regex("car", "c.t").is_none());
    assert_eq!(match_regex("goøö0Ogol", "g.+gol"), Some("ggol".to_string()));
    assert!(match_regex("gol", "g.+gol").is_none());
}

#[test]
fn digit_matching() {
    assert_eq!(match_regex("123", r"\d"), Some("1".to_string()));
    assert!(match_regex("apple", r"\d").is_none());
    assert_eq!(match_regex("abc_0_xyz", r"\d"), Some("0".to_string()));
}

#[test]
fn word_character_matching() {
    assert_eq!(match_regex("grape", r"\w"), Some("g".to_string()));
    assert_eq!(match_regex("RASPBERRY", r"\w"), Some("R".to_string()));
    assert_eq!(match_regex("303", r"\w"), Some("3".to_string()));
    assert_eq!(match_regex("-#+_-+×", r"\w"), Some("_".to_string()));
    assert!(match_regex("%#=÷-+", r"\w").is_none());
}

#[test]
fn positive_character_groups() {
    assert_eq!(match_regex("b", "[strawberry]"), Some("b".to_string()));
    assert_eq!(match_regex("bcd", "[strawberry]"), Some("b".to_string()));
    assert!(match_regex("pineapple", "[bcdfghjkm]").is_none());
    assert!(match_regex("[]", "[mango]").is_none());
}

#[test]
fn negative_character_groups() {
    assert_eq!(match_regex("apple", "[^xyz]"), Some("a".to_string()));
    assert_eq!(match_regex("apple", "[^abc]"), Some("p".to_string()));
    assert!(match_regex("banana", "[^anb]").is_none());
    assert_eq!(match_regex("orange", "[^opq]"), Some("r".to_string()));
}

#[test]
fn combining_character_classes() {
    assert_eq!(
        match_regex("sally has 3 apples", r"\d apple"),
        Some("3 apple".to_string())
    );
    assert!(match_regex("sally has 1 orange", r"\d apple").is_none());
    assert_eq!(
        match_regex("sally has 124 apples", r"\d\d\d apples"),
        Some("124 apples".to_string())
    );
    assert!(match_regex("sally has 12 apples", r"\d\\d\\d apples").is_none());
    assert_eq!(
        match_regex("sally has 3 dogs", r"\d \w\w\ws"),
        Some("3 dogs".to_string())
    );
    assert_eq!(
        match_regex("sally has 4 dogs", r"\d \w\w\ws"),
        Some("4 dogs".to_string())
    );
    assert!(match_regex("sally has 1 dog", r"\d \w\w\ws").is_none());
}

#[test]
fn anchors() {
    // Start anchor
    assert_eq!(
        match_regex("mango_apple", "^mango"),
        Some("mango".to_string())
    );
    assert!(match_regex("apple_mango", "^mango").is_none());

    // End anchor
    assert_eq!(
        match_regex("strawberry_blueberry", "blueberry$"),
        Some("blueberry".to_string())
    );
    assert!(match_regex("blueberry_strawberry", "blueberry$").is_none());

    // Both anchors
    assert_eq!(match_regex("pear", "^pear$"), Some("pear".to_string()));
    assert!(match_regex("pear_pear", "^pear$").is_none());
}

#[test]
fn quantifiers_zero_or_one() {
    assert_eq!(match_regex("cat", "ca?t"), Some("cat".to_string()));
    assert_eq!(match_regex("act", "ca?t"), Some("ct".to_string()));
    assert_eq!(match_regex("cat", "ca?a?t"), Some("cat".to_string()));
    assert!(match_regex("dog", "ca?t").is_none());
    assert!(match_regex("cag", "ca?t").is_none());
}

#[test]
fn quantifiers_one_or_more() {
    assert_eq!(match_regex("cat", "ca+t"), Some("cat".to_string()));
    assert_eq!(match_regex("caaats", "ca+at"), Some("cat".to_string()));
    assert!(match_regex("act", "ca+t").is_none());
    assert!(match_regex("ca", "ca+t").is_none());
    assert_eq!(
        match_regex("abc_123_xyz", r"^abc_\d+_xyz$"),
        Some("abc_123_xyz".to_string())
    );
    assert!(match_regex("abc_rst_xyz", r"^abc_\d+_xyz$").is_none());
}

#[test]
fn quantifiers_zero_or_more() {
    assert_eq!(match_regex("apple", "apple*"), Some("apple".to_string()));
    assert_eq!(match_regex("appl", "apple*"), Some("appl".to_string()));
    assert_eq!(
        match_regex("horse54tiger", r"horse\d*tiger"),
        Some("horse54tiger".to_string())
    );
    assert_eq!(
        match_regex("peas,_fresh", r"peas,\w*"),
        Some("peas,_fresh".to_string())
    );
    assert_eq!(
        match_regex("tigerhorsehorsetiger", "(horse|tiger)*"),
        Some("tigerhorsehorsetiger".to_string())
    );
    assert_eq!(
        match_regex("LOG INFO 21 apple", r"^LOG [FION]* \d+ (apple|peas)$"),
        Some("LOG INFO 21 apple".to_string())
    );
    assert!(match_regex("LOG info 85 apple", r"^LOG [FION]* \d+ (apple|peas)$").is_none());
}

#[test]
fn quantifiers_exact_count() {
    assert_eq!(match_regex("caaat", r"ca{3}t"), Some("caaat".to_string()));
    assert!(match_regex("caat", r"ca{3}t").is_none());
    assert!(match_regex("caaaat", r"ca{3}t").is_none());
    assert_eq!(match_regex("d42g", r"d\d{2}g"), Some("d42g".to_string()));
    assert!(match_regex("d1g", r"d\d{2}g").is_none());
    assert!(match_regex("d123g", r"d\d{2}g").is_none());
    assert_eq!(
        match_regex("czyxzw", r"c[xyz]{4}w"),
        Some("czyxzw".to_string())
    );
    assert!(match_regex("cxyzw", r"c[xyz]{4}w").is_none());
}

#[test]
fn quantifiers_at_least_count() {
    assert_eq!(match_regex("caat", r"ca{2,}t"), Some("caat".to_string()));
    assert_eq!(
        match_regex("caaaaat", r"ca{2,}t"),
        Some("caaaaat".to_string())
    );
    assert!(match_regex("cat", r"ca{2,}t").is_none());
    assert_eq!(
        match_regex("x9999y", r"x\d{3,}y"),
        Some("x9999y".to_string())
    );
    assert!(match_regex("x42y", r"x\d{3,}y").is_none());
    assert_eq!(
        match_regex("baeiour", r"b[aeiou]{2,}r"),
        Some("baeiour".to_string())
    );
    assert!(match_regex("bar", r"b[aeiou]{2,}r").is_none());
}

#[test]
fn alternation_basic() {
    assert_eq!(
        match_regex("a cat", "a (cat|dog)"),
        Some("a cat".to_string())
    );
    assert!(match_regex("a cog", "a (cat|dog)").is_none());
    assert_eq!(
        match_regex("I see 1 cat", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 1 cat".to_string())
    );
    assert_eq!(
        match_regex("I see 42 dogs", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 42 dogs".to_string())
    );
    assert!(match_regex("I see a cat", r"^I see \d+ (cat|dog)s?$").is_none());
    assert!(match_regex("I see 2 dog3", r"^I see \d+ (cat|dog)s?$").is_none());
}

#[test]
fn single_backreferences() {
    assert_eq!(
        match_regex("cat and cat", "(cat) and \\1"),
        Some("cat and cat".to_string())
    );
    assert!(match_regex("cat and dog", "(cat) and \\1").is_none());
    assert_eq!(
        match_regex("cat and cat", r"(\w+) and \1"),
        Some("cat and cat".to_string())
    );
    assert!(match_regex("cat and dog", r"(\w+) and \1").is_none());
    assert_eq!(
        match_regex("cat is cat, not dog", r"^([act]+) is \1, not [^xyz]+$"),
        Some("cat is cat, not dog".to_string())
    );
    assert!(match_regex("cat is c@t, not d0g", r"^([act]+) is \1, not [^xyz]+$").is_none());
}

#[test]
fn multiple_backreferences() {
    assert_eq!(
        match_regex(
            "3 red squares and 3 red circles",
            r"(\d+) (\w+) squares and \1 \2 circles"
        ),
        Some("3 red squares and 3 red circles".to_string())
    );
    assert!(match_regex(
        "3 red squares and 4 red circles",
        r"(\d+) (\w+) squares and \1 \2 circles"
    )
    .is_none());
    assert_eq!(
        match_regex(
            "grep 101 is doing grep 101 times",
            r"(\w\w\w\w) (\d\d\d) is doing \1 \2 times"
        ),
        Some("grep 101 is doing grep 101 times".to_string())
    );
    assert!(match_regex(
        "$?! 101 is doing $?! 101 times",
        r"(\w\w\w) (\d\d\d) is doing \1 \2 times"
    )
    .is_none());
    assert!(match_regex(
        "grep yes is doing grep yes times",
        r"(\w\w\w\w) (\d\d\d) is doing \1 \2 times"
    )
    .is_none());
    assert_eq!(
        match_regex(
            "abc-def is abc-def, not efg",
            r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
        ),
        Some("abc-def is abc-def, not efg".to_string())
    );
    assert!(match_regex(
        "efg-hij is efg-hij, not efg",
        r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
    )
    .is_none());
    assert!(match_regex(
        "abc-def is abc-def, not xyz",
        r"([abc]+)-([def]+) is \1-\2, not [^xyz]+"
    )
    .is_none());
    assert_eq!(
        match_regex("apple pie, apple and pie", r"^(\w+) (\w+), \1 and \2$"),
        Some("apple pie, apple and pie".to_string())
    );
    assert!(match_regex(
        "pineapple pie, pineapple and pie",
        r"^(apple) (\w+), \1 and \2$"
    )
    .is_none());
    assert!(match_regex("apple pie, apple and pies", r"^(\w+) (pie), \1 and \2$").is_none());
    assert_eq!(
        match_regex(
            "howwdy hey there, howwdy hey",
            r"(how+dy) (he?y) there, \1 \2"
        ),
        Some("howwdy hey there, howwdy hey".to_string())
    );
    assert!(match_regex(
        "hody hey there, howwdy hey",
        r"(how+dy) (he?y) there, \1 \2"
    )
    .is_none());
    assert!(match_regex(
        "howwdy heeey there, howwdy heeey",
        r"(how+dy) (he?y) there, \1 \2"
    )
    .is_none());
    assert_eq!(
        match_regex(
            "cat and fish, cat with fish",
            r"(c.t|d.g) and (f..h|b..d), \1 with \2"
        ),
        Some("cat and fish, cat with fish".to_string())
    );
    assert!(match_regex(
        "bat and fish, cat with fish",
        r"(c.t|d.g) and (f..h|b..d), \1 with \2"
    )
    .is_none());
}

#[test]
fn complex_quantifier_combinations() {
    // Test the specific case that was failing before
    assert_eq!(
        match_regex("pandadogdogpanda", "(dog|panda)*"),
        Some("pandadogdogpanda".to_string())
    );

    // Test digit patterns with quantifiers
    assert_eq!(
        match_regex("I see 1 cat", r"^I see \d+ (cat|dog)s?$"),
        Some("I see 1 cat".to_string())
    );
    assert!(match_regex("I see a cat", r"^I see \d+ (cat|dog)s?$").is_none());

    // Test character groups with quantifiers
    assert_eq!(
        match_regex("LOG INFO 21 apple", r"^LOG [FION]* \d+ (apple|peas)$"),
        Some("LOG INFO 21 apple".to_string())
    );
    assert!(match_regex("LOG info 85 apple", r"^LOG [FION]* \d+ (apple|peas)$").is_none());
}
