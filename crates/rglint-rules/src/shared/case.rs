use super::case_styles::CaseStyle;

pub fn split_words(name: &str) -> Vec<String> {
    if name.is_empty() {
        return vec![];
    }

    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = name.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if !c.is_ascii_alphanumeric() {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            i += 1;
            continue;
        }

        if current.is_empty() {
            current.push(c);
            i += 1;
            continue;
        }

        let prev = current.chars().last().unwrap();

        let should_split = prev.is_ascii_lowercase() && c.is_ascii_uppercase()
            || prev.is_ascii_alphabetic() && c.is_ascii_digit()
            || prev.is_ascii_digit() && c.is_ascii_alphabetic();

        if should_split {
            words.push(current.clone());
            current.clear();
            current.push(c);
            i += 1;
            continue;
        }

        if prev.is_ascii_uppercase() && c.is_ascii_lowercase() && current.len() > 1 {
            let all_upper_so_far = current.chars().all(|ch| ch.is_ascii_uppercase());
            if all_upper_so_far {
                let last = current.pop().unwrap();
                words.push(current.clone());
                current.clear();
                current.push(last);
                current.push(c);
                i += 1;
                continue;
            }
        }

        current.push(c);
        i += 1;
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn capitalize_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    let mut chars: Vec<char> = lower.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_uppercase();
    }
    chars.into_iter().collect()
}

pub fn detect_case(name: &str) -> Option<CaseStyle> {
    if name.is_empty() {
        return None;
    }

    let has_hyphen = name.contains('-');
    let has_underscore = name.contains('_');

    let all_upper = name
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .all(|c| c.is_ascii_uppercase());
    let all_lower = name
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .all(|c| c.is_ascii_lowercase());
    let first_upper = name.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    let first_lower = name.chars().next().is_some_and(|c| c.is_ascii_lowercase());

    if has_hyphen {
        if all_upper {
            return Some(CaseStyle::ScreamingKebab);
        } else if all_lower {
            return Some(CaseStyle::Kebab);
        }
        return None;
    }

    if has_underscore {
        if all_upper {
            return Some(CaseStyle::ScreamingSnake);
        } else if all_lower {
            return Some(CaseStyle::Snake);
        }
        return None;
    }

    if first_upper && !all_upper {
        let has_consecutive_upper = name
            .as_bytes()
            .windows(2)
            .any(|w| w[0].is_ascii_uppercase() && w[1].is_ascii_uppercase());

        let has_transition = name
            .as_bytes()
            .windows(2)
            .any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase());

        if !has_consecutive_upper && has_transition {
            return Some(CaseStyle::StrictPascal);
        }
        return Some(CaseStyle::Pascal);
    }

    if first_upper && all_upper {
        return Some(CaseStyle::Pascal);
    }

    if first_lower && !has_underscore && !has_hyphen {
        return Some(CaseStyle::Camel);
    }

    None
}

pub fn is_case(name: &str, style: CaseStyle) -> bool {
    detect_case(name) == Some(style)
}

pub fn convert_case(name: &str, style: CaseStyle, acronyms: &[String]) -> String {
    if name.is_empty() {
        return String::new();
    }

    let words = split_words(name);
    if words.is_empty() {
        return String::new();
    }

    let transformed: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, word)| match style {
            CaseStyle::Camel => {
                if i == 0 {
                    word.to_ascii_lowercase()
                } else {
                    capitalize_word(word)
                }
            }
            CaseStyle::Pascal => capitalize_word(word),
            CaseStyle::StrictPascal => {
                let upper = word.to_ascii_uppercase();
                if acronyms.iter().any(|a| a.as_str() == upper) {
                    upper
                } else {
                    capitalize_word(word)
                }
            }
            CaseStyle::Snake => word.to_ascii_lowercase(),
            CaseStyle::ScreamingSnake => word.to_ascii_uppercase(),
            CaseStyle::Kebab => word.to_ascii_lowercase(),
            CaseStyle::ScreamingKebab => word.to_ascii_uppercase(),
        })
        .collect();

    match style {
        CaseStyle::Camel | CaseStyle::Pascal | CaseStyle::StrictPascal => transformed.join(""),
        CaseStyle::Snake | CaseStyle::ScreamingSnake => transformed.join("_"),
        CaseStyle::Kebab | CaseStyle::ScreamingKebab => transformed.join("-"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acr(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn split_words_empty() {
        assert!(split_words("").is_empty());
    }

    #[test]
    fn split_words_camel_case() {
        assert_eq!(split_words("fooBar"), vec!["foo", "Bar"]);
    }

    #[test]
    fn split_words_pascal_case() {
        assert_eq!(split_words("FooBar"), vec!["Foo", "Bar"]);
    }

    #[test]
    fn split_words_with_numbers() {
        assert_eq!(split_words("foo2Bar"), vec!["foo", "2", "Bar"]);
    }

    #[test]
    fn split_words_snake_case() {
        assert_eq!(split_words("foo_bar"), vec!["foo", "bar"]);
    }

    #[test]
    fn split_words_screaming_snake() {
        assert_eq!(split_words("FOO_BAR"), vec!["FOO", "BAR"]);
    }

    #[test]
    fn split_words_kebab() {
        assert_eq!(split_words("foo-bar"), vec!["foo", "bar"]);
    }

    #[test]
    fn split_words_screaming_kebab() {
        assert_eq!(split_words("FOO-BAR"), vec!["FOO", "BAR"]);
    }

    #[test]
    fn split_words_consecutive_uppercase() {
        assert_eq!(split_words("FOOBar"), vec!["FOO", "Bar"]);
    }

    #[test]
    fn split_words_single_word() {
        assert_eq!(split_words("foo"), vec!["foo"]);
    }

    #[test]
    fn split_words_digits_only() {
        assert_eq!(split_words("123"), vec!["123"]);
    }

    #[test]
    fn split_words_triple_upper() {
        assert_eq!(split_words("FOO"), vec!["FOO"]);
    }

    #[test]
    fn split_words_acronym() {
        assert_eq!(split_words("URL"), vec!["URL"]);
    }

    #[test]
    fn detect_empty() {
        assert_eq!(detect_case(""), None);
    }

    #[test]
    fn detect_camel_case() {
        assert_eq!(detect_case("foo"), Some(CaseStyle::Camel));
        assert_eq!(detect_case("fooBar"), Some(CaseStyle::Camel));
        assert_eq!(detect_case("foo2Bar"), Some(CaseStyle::Camel));
        assert_eq!(detect_case("fooBarBaz"), Some(CaseStyle::Camel));
    }

    #[test]
    fn detect_pascal_case() {
        assert_eq!(detect_case("Foo"), Some(CaseStyle::Pascal));
        assert_eq!(detect_case("FOOBar"), Some(CaseStyle::Pascal));
        assert_eq!(detect_case("UserURL"), Some(CaseStyle::Pascal));
        assert_eq!(detect_case("URL"), Some(CaseStyle::Pascal));
    }

    #[test]
    fn detect_strict_pascal() {
        assert_eq!(detect_case("FooBar"), Some(CaseStyle::StrictPascal));
        assert_eq!(detect_case("FooBarBaz"), Some(CaseStyle::StrictPascal));
    }

    #[test]
    fn detect_snake_case() {
        assert_eq!(detect_case("foo_bar"), Some(CaseStyle::Snake));
        assert_eq!(detect_case("foo_bar_baz"), Some(CaseStyle::Snake));
        assert_eq!(detect_case("foo_2_bar"), Some(CaseStyle::Snake));
    }

    #[test]
    fn detect_screaming_snake() {
        assert_eq!(detect_case("FOO_BAR"), Some(CaseStyle::ScreamingSnake));
        assert_eq!(detect_case("FOO_BAR_BAZ"), Some(CaseStyle::ScreamingSnake));
    }

    #[test]
    fn detect_kebab() {
        assert_eq!(detect_case("foo-bar"), Some(CaseStyle::Kebab));
        assert_eq!(detect_case("foo-bar-baz"), Some(CaseStyle::Kebab));
    }

    #[test]
    fn detect_screaming_kebab() {
        assert_eq!(detect_case("FOO-BAR"), Some(CaseStyle::ScreamingKebab));
    }

    #[test]
    fn detect_mixed_case_with_separator_returns_none() {
        assert_eq!(detect_case("foo_Bar"), None);
        assert_eq!(detect_case("FOO_bar"), None);
        assert_eq!(detect_case("foo-Bar"), None);
        assert_eq!(detect_case("FOO-bar"), None);
    }

    #[test]
    fn is_case_true() {
        assert!(is_case("fooBar", CaseStyle::Camel));
        assert!(is_case("FooBar", CaseStyle::StrictPascal));
        assert!(is_case("FOOBar", CaseStyle::Pascal));
        assert!(is_case("foo_bar", CaseStyle::Snake));
        assert!(is_case("FOO_BAR", CaseStyle::ScreamingSnake));
        assert!(is_case("foo-bar", CaseStyle::Kebab));
        assert!(is_case("FOO-BAR", CaseStyle::ScreamingKebab));
    }

    #[test]
    fn is_case_false() {
        assert!(!is_case("foo_bar", CaseStyle::Camel));
        assert!(!is_case("FooBar", CaseStyle::Camel));
        assert!(!is_case("fooBar", CaseStyle::Snake));
    }

    #[test]
    fn convert_empty() {
        assert_eq!(convert_case("", CaseStyle::Camel, &[]), "");
    }

    #[test]
    fn convert_to_camel() {
        assert_eq!(convert_case("foo_bar", CaseStyle::Camel, &[]), "fooBar");
        assert_eq!(convert_case("FOO_BAR", CaseStyle::Camel, &[]), "fooBar");
        assert_eq!(convert_case("foo-bar", CaseStyle::Camel, &[]), "fooBar");
        assert_eq!(convert_case("FooBar", CaseStyle::Camel, &[]), "fooBar");
        assert_eq!(convert_case("FOOBar", CaseStyle::Camel, &[]), "fooBar");
        assert_eq!(convert_case("foo", CaseStyle::Camel, &[]), "foo");
    }

    #[test]
    fn convert_to_pascal() {
        assert_eq!(convert_case("foo_bar", CaseStyle::Pascal, &[]), "FooBar");
        assert_eq!(convert_case("fooBar", CaseStyle::Pascal, &[]), "FooBar");
        assert_eq!(convert_case("FOO_BAR", CaseStyle::Pascal, &[]), "FooBar");
        assert_eq!(convert_case("foo", CaseStyle::Pascal, &[]), "Foo");
        assert_eq!(convert_case("URL", CaseStyle::Pascal, &[]), "Url");
    }

    #[test]
    fn convert_to_snake() {
        assert_eq!(convert_case("fooBar", CaseStyle::Snake, &[]), "foo_bar");
        assert_eq!(convert_case("FooBar", CaseStyle::Snake, &[]), "foo_bar");
        assert_eq!(convert_case("FOOBar", CaseStyle::Snake, &[]), "foo_bar");
        assert_eq!(convert_case("FOO-BAR", CaseStyle::Snake, &[]), "foo_bar");
    }

    #[test]
    fn convert_to_screaming_snake() {
        assert_eq!(
            convert_case("fooBar", CaseStyle::ScreamingSnake, &[]),
            "FOO_BAR"
        );
        assert_eq!(
            convert_case("FooBar", CaseStyle::ScreamingSnake, &[]),
            "FOO_BAR"
        );
        assert_eq!(
            convert_case("foo-bar", CaseStyle::ScreamingSnake, &[]),
            "FOO_BAR"
        );
    }

    #[test]
    fn convert_to_kebab() {
        assert_eq!(convert_case("fooBar", CaseStyle::Kebab, &[]), "foo-bar");
        assert_eq!(convert_case("FooBar", CaseStyle::Kebab, &[]), "foo-bar");
        assert_eq!(convert_case("foo_bar", CaseStyle::Kebab, &[]), "foo-bar");
    }

    #[test]
    fn convert_to_screaming_kebab() {
        assert_eq!(
            convert_case("fooBar", CaseStyle::ScreamingKebab, &[]),
            "FOO-BAR"
        );
        assert_eq!(
            convert_case("FooBar", CaseStyle::ScreamingKebab, &[]),
            "FOO-BAR"
        );
        assert_eq!(
            convert_case("foo_bar", CaseStyle::ScreamingKebab, &[]),
            "FOO-BAR"
        );
    }

    #[test]
    fn convert_to_strict_pascal() {
        assert_eq!(
            convert_case("foo_bar", CaseStyle::StrictPascal, &[]),
            "FooBar"
        );
        assert_eq!(
            convert_case("fooBar", CaseStyle::StrictPascal, &[]),
            "FooBar"
        );
    }

    #[test]
    fn convert_with_acronyms() {
        assert_eq!(
            convert_case("user_url", CaseStyle::StrictPascal, &acr(&["URL"])),
            "UserURL"
        );
        assert_eq!(
            convert_case("user_url", CaseStyle::StrictPascal, &acr(&["Url"])),
            "UserUrl"
        );
        assert_eq!(
            convert_case("url", CaseStyle::StrictPascal, &acr(&["URL"])),
            "URL"
        );
    }

    #[test]
    fn convert_acronyms_not_applied_to_pascal() {
        assert_eq!(
            convert_case("user_url", CaseStyle::Pascal, &acr(&["URL"])),
            "UserUrl"
        );
    }

    #[test]
    fn roundtrip_camel() {
        let input = "fooBar";
        let converted = convert_case(input, CaseStyle::Camel, &[]);
        assert_eq!(converted, "fooBar");
    }

    #[test]
    fn roundtrip_snake() {
        let input = "foo_bar";
        let converted = convert_case(input, CaseStyle::Snake, &[]);
        assert_eq!(converted, "foo_bar");
    }

    #[test]
    fn detect_case_single_letter() {
        assert_eq!(detect_case("a"), Some(CaseStyle::Camel));
        assert_eq!(detect_case("A"), Some(CaseStyle::Pascal));
    }

    #[test]
    fn namespace_reexports_work() {
        let words = super::super::case::split_words("helloWorld");
        assert_eq!(words, vec!["hello", "World"]);
    }

    proptest::proptest! {
        #[test]
        fn proptest_idempotence_convert_snake(s in "[a-zA-Z][a-zA-Z0-9_\\-]*") {
            let first = convert_case(&s, CaseStyle::Snake, &[]);
            let second = convert_case(&first, CaseStyle::Snake, &[]);
            assert_eq!(first, second, "convert_case(Snake) must be idempotent for input: {}", s);
        }

        #[test]
        fn proptest_idempotence_convert_screaming_snake(s in "[a-zA-Z][a-zA-Z0-9_\\-]*") {
            let first = convert_case(&s, CaseStyle::ScreamingSnake, &[]);
            let second = convert_case(&first, CaseStyle::ScreamingSnake, &[]);
            assert_eq!(first, second, "convert_case(ScreamingSnake) must be idempotent for input: {}", s);
        }

        #[test]
        fn proptest_idempotence_convert_kebab(s in "[a-zA-Z][a-zA-Z0-9_\\-]*") {
            let first = convert_case(&s, CaseStyle::Kebab, &[]);
            let second = convert_case(&first, CaseStyle::Kebab, &[]);
            assert_eq!(first, second, "convert_case(Kebab) must be idempotent for input: {}", s);
        }

        #[test]
        fn proptest_idempotence_convert_screaming_kebab(s in "[a-zA-Z][a-zA-Z0-9_\\-]*") {
            let first = convert_case(&s, CaseStyle::ScreamingKebab, &[]);
            let second = convert_case(&first, CaseStyle::ScreamingKebab, &[]);
            assert_eq!(first, second, "convert_case(ScreamingKebab) must be idempotent for input: {}", s);
        }

        #[test]
        fn proptest_roundtrip_detect_camel(s in "[a-z][a-zA-Z0-9]*") {
            let converted = convert_case(&s, CaseStyle::Camel, &[]);
            assert!(detect_case(&converted) == Some(CaseStyle::Camel) || detect_case(&converted) == Some(CaseStyle::Pascal),
                "detect_case(convert_case({}, Camel))={:?} should be Camel or Pascal", s, detect_case(&converted));
        }

        #[test]
        fn proptest_roundtrip_detect_pascal(s in "[A-Z][a-zA-Z0-9]*") {
            let converted = convert_case(&s, CaseStyle::Pascal, &[]);
            assert!(detect_case(&converted) == Some(CaseStyle::Pascal) || detect_case(&converted) == Some(CaseStyle::StrictPascal),
                "detect_case(convert_case({}, Pascal))={:?} should be Pascal or StrictPascal", s, detect_case(&converted));
        }
    }
}
