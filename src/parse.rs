//! Parsing primitives

use nom::{
    IResult, Parser, branch::alt, character::complete::one_of,
    combinator::recognize, multi::many1, sequence::pair,
};

/// Match WikiWords
pub fn wiki_word(s: &str) -> Option<&str> {
    fn word_segment(s: &str) -> IResult<&str, &str> {
        recognize(pair(
            one_of("ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            many1(one_of("abcdefghijklmnopqrstuvwxyz")),
        ))
        .parse(s)
    }

    fn number_segment(s: &str) -> IResult<&str, &str> {
        recognize(many1(one_of("0123456789"))).parse(s)
    }

    let Ok((rest, word)) = recognize(pair(
        word_segment,
        many1(alt((word_segment, number_segment))),
    ))
    .parse(s) else {
        return None;
    };

    if rest.is_empty() { Some(word) } else { None }
}

/// Match important headlines that end with " *", return the part before the
/// importance marker.
pub fn important(s: &str) -> Option<&str> {
    s.strip_suffix(" *")
}

pub fn indentation(s: &str) -> &str {
    let Some(i) = s
        .char_indices()
        .find_map(|(i, c)| (!c.is_whitespace()).then_some(i))
    else {
        return "";
    };
    &s[0..i]
}

/// Convert a `CamelCase` string to `kebab-case`.
pub fn camel_to_kebab(input: &str) -> String {
    let mut kebab = String::new();
    let mut last = '-';

    for c in input.chars() {
        if last != '-'
            && (c.is_uppercase() || c.is_numeric() != last.is_numeric())
        {
            kebab.push('-');
        }

        kebab.push(c.to_ascii_lowercase());
        last = c;
    }

    kebab
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiki_word() {
        assert_eq!(wiki_word("WikiWord"), Some("WikiWord"));
        assert_eq!(wiki_word("WikiWord666"), Some("WikiWord666"));
        assert_eq!(wiki_word("Wiki666"), Some("Wiki666"));
        assert_eq!(wiki_word("Wiki"), None);
        assert_eq!(wiki_word("wikiWord"), None);
        assert_eq!(wiki_word("666Wiki"), None);
        assert_eq!(wiki_word("WikiWord-"), None);
        assert_eq!(wiki_word(""), None);
        assert_eq!(wiki_word("Wiki"), None);
    }

    #[test]
    fn test_important() {
        assert_eq!(important("Important *"), Some("Important"));
        assert_eq!(important("Important * "), None);
        assert_eq!(important("Important"), None);
    }

    #[test]
    fn test_camel_to_kebab() {
        assert_eq!(camel_to_kebab("CamelCase"), "camel-case");
        assert_eq!(camel_to_kebab("Camel"), "camel");
        assert_eq!(camel_to_kebab("CamelCase666"), "camel-case-666");
        assert_eq!(camel_to_kebab("666Camel"), "666-camel");
    }
}
