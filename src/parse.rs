//! Parsing primitives

use lazy_regex::regex;
use nom::{
    branch::alt, character::complete::one_of, combinator::recognize,
    multi::many1, sequence::pair, IResult, Parser,
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

    if rest.is_empty() {
        Some(word)
    } else {
        None
    }
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

/// Try to merge differntly formatted URLs pointing to the same thing into one identifier.
pub fn normalized_url(url: &str) -> String {
    let url = url.trim();

    let wayback_re =
        regex!(r"^https?://web\.archive\.org/web/\d+/(https?://.+)$");
    let archive_re =
        regex!(r"^https?://archive\.(is|md|li|ph|today)/.+/(https?://.+)$");

    // Recursively remove archiver wrappers.
    if let Some(caps) = wayback_re.captures(url) {
        return normalized_url(&caps[1]);
    }
    if let Some(caps) = archive_re.captures(url) {
        return normalized_url(&caps[2]);
    }

    // Various twitters and twitter-wrappers. We're assuming the numbers are unique so the username
    // can be discarded.
    let twitter_re = regex!(r"^https?://twitter\.com/.+/status/(\d+)$");
    let xcom_re = regex!(r"^https?://x\.com/.+/status/(\d+)$");
    let nitter_re =
        regex!(r"^https?://nitter\.(net|poast.org)/.+/status/(\d+)$");
    let xcancel_re = regex!(r"^https?://xcancel\.com/.+/status/(\d+)$");
    let threadreader_re =
        regex!(r"^https?://threadreaderapp\.com/thread/(\d+)(.html)?$");

    if let Some(caps) = twitter_re.captures(url) {
        return format!("https://twitter.com/a/status/{}", &caps[1]);
    }
    if let Some(caps) = xcom_re.captures(url) {
        return format!("https://twitter.com/a/status/{}", &caps[1]);
    }
    if let Some(caps) = nitter_re.captures(url) {
        return format!("https://twitter.com/a/status/{}", &caps[2]);
    }
    if let Some(caps) = xcancel_re.captures(url) {
        return format!("https://twitter.com/a/status/{}", &caps[1]);
    }
    if let Some(caps) = threadreader_re.captures(url) {
        return format!("https://twitter.com/a/status/{}", &caps[1]);
    }

    // Turn name.tumblr.com/post/ into www.tumblr.com/name/
    let tumblr_re =
        regex!(r"^https?://([a-zA-Z0-9_-]+)\.tumblr\.com/post/(.+)$");
    if let Some(caps) = tumblr_re.captures(url) {
        return format!("https://www.tumblr.com/{}/{}", &caps[1], &caps[2]);
    }

    // Force everything to https.
    if url.starts_with("http://") {
        format!("https://{}", &url[7..])
    } else {
        url.to_string()
    }
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

    #[test]
    fn test_normalized_url() {
        for (input, expected) in vec![
            ("http://example.com", "https://example.com"),
            ("https://example.com", "https://example.com"),
            (
                "http://web.archive.org/web/20220101010101/http://example.com",
                "https://example.com",
            ),
            (
                "https://archive.is/XYZ123/http://example.com",
                "https://example.com",
            ),
            (
                "https://archive.today/XYZ123/http://example.com",
                "https://example.com",
            ),
            (
                "https://archive.ph/XYZ123/http://example.com",
                "https://example.com",
            ),
            (
                "https://twitter.com/someuser/status/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://x.com/someuser/status/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://nitter.poast.org/someuser/status/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://nitter.net/someuser/status/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://xcancel.com/someuser/status/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://threadreaderapp.com/thread/1234567890",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://threadreaderapp.com/thread/1234567890.html",
                "https://twitter.com/a/status/1234567890",
            ),
            (
                "https://someblog.tumblr.com/post/12345/blag",
                "https://www.tumblr.com/someblog/12345/blag",
            ),
        ] {
            assert_eq!(normalized_url(input), expected);
        }
    }
}
