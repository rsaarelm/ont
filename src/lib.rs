use std::{fs, path::Path};

use anyhow::{bail, Result};

mod collection;
pub use collection::{read_directory, write_directory};

pub mod parse;

mod outline;
use idm::ser::Indentation;
use lazy_regex::regex;
pub use outline::{Outline, Section, SimpleOutline, SimpleSection};

pub fn read_outline(path: impl AsRef<Path>) -> Result<(Outline, Indentation)> {
    use std::io::Read;

    let path = path.as_ref();

    if path.to_str() == Some("-") {
        // Read stdin to string.
        let mut content = String::new();
        std::io::stdin().read_to_string(&mut content)?;
        let style = Indentation::infer(&content).unwrap_or_default();
        Ok((idm::from_str(&content)?, style))
    } else if path.is_file() {
        // Read file.
        let content = std::fs::read_to_string(path)?;
        let style = Indentation::infer(&content).unwrap_or_default();
        Ok((idm::from_str(&content)?, style))
    } else if path.is_dir() {
        // Read collection directory.
        let (outline, style, _) = read_directory(path)?;
        Ok((outline, style))
    } else {
        bail!("read_outline: invalid path {path:?}");
    }
}

/// Delete file at `path` and any empty subdirectories between `root` and
/// `path`.
///
/// This is a git-style file delete that deletes the containing subdirectory
/// if it's emptied of files.
pub fn tidy_delete(root: &Path, mut path: &Path) -> Result<()> {
    fs::remove_file(path)?;
    log::debug!("tidy_delete: Deleted {path:?}");

    while let Some(parent) = path.parent() {
        // Keep going up the subdirectories...
        path = parent;

        // ...that are underneath `root`...
        if !path.starts_with(root)
            || path.components().count() <= root.components().count()
        {
            break;
        }

        // ...and deleting them if you can.
        if fs::remove_dir(path).is_ok() {
            log::debug!("tidy_delete: Deleted empty subdirectory {path:?}");
        } else {
            // We couldn't delete the directory, assume it wasn't empty and
            // that there's no point going further up the tree.
            break;
        }
    }

    Ok(())
}

pub fn web_url(s: &Section) -> Option<String> {
    let uri: String = s.body.get::<String>("uri").unwrap()?;
    // Gonna ignore ftp: and other weird 90s stuff.
    if uri.starts_with("http:") || uri.starts_with("https:") {
        Some(uri)
    } else {
        None
    }
}

/// Try to merge differntly formatted URLs pointing to the same thing into one identifier.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();

    let wayback_re =
        regex!(r"^https?://web\.archive\.org/web/\d+/(https?://.+)$");
    let archive_re =
        regex!(r"^https?://archive\.(is|md|li|ph|today)/.+/(https?://.+)$");

    // Recursively remove archiver wrappers.
    if let Some(caps) = wayback_re.captures(url) {
        return normalize_url(&caps[1]);
    }
    if let Some(caps) = archive_re.captures(url) {
        return normalize_url(&caps[2]);
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
    fn test_normalize_url() {
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
            assert_eq!(normalize_url(input), expected);
        }
    }
}
