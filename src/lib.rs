use std::{
    collections::HashMap,
    fmt,
    fs::{self, File},
    io::{self, prelude::*},
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use lazy_regex::regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use walkdir::WalkDir;

pub type Section = ((String,), Outline);

/// An outline with a header section.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outline(pub (IndexMap<String, String>,), pub Vec<Section>);

pub type SimpleSection = ((String,), SimpleOutline);

/// An outline without a separately parsed header section.
///
/// Most general form of parsing an IDM document.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimpleOutline(pub Vec<SimpleSection>);

impl fmt::Display for SimpleOutline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            outline: &SimpleOutline,
        ) -> fmt::Result {
            for ((head,), body) in &outline.0 {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                writeln!(f, "{head}")?;
                print(f, depth + 1, body)?;
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

impl Outline {
    fn transform_inner<C: Default + Clone>(
        &mut self,
        ctx: C,
        f: &mut impl FnMut(C, Section) -> (C, Vec<Section>),
    ) {
        let mut n = 0;
        while n < self.1.len() {
            let mut sec = Section::default();
            std::mem::swap(&mut sec, &mut self.1[n]);
            let (new_ctx, mut otl) = f(ctx.clone(), sec);

            let span = if otl.is_empty() {
                // Removed a section.
                self.1.swap_remove(n);
                0
            } else if otl.len() == 1 {
                // Replaced a section.
                self.1[n] = otl.pop().unwrap();
                1
            } else {
                // Expansion into multiple sections.
                let tail = self.1.split_off(n);
                let len = otl.len();
                self.1.extend(otl);
                self.1.extend(tail);
                len
            };

            // Recurse to new sections.
            for i in n..(n + span) {
                self.1[i].1.transform_inner(new_ctx.clone(), f);
            }
            n += span;
        }
    }

    pub fn transform<C: Default + Clone>(
        &mut self,
        mut f: impl FnMut(C, Section) -> (C, Vec<Section>),
    ) {
        self.transform_inner(Default::default(), &mut f);
    }
}

pub fn read_directory<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T> {
    use std::fmt::Write;

    let mut ret = String::new();
    for e in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let depth = e.depth();
        if depth == 0 {
            // The root element, do not print out.
            continue;
        }
        for _ in 1..depth {
            write!(ret, "  ")?;
        }
        let is_dir = e.file_type().is_dir();
        let file_name = e.file_name().to_string_lossy();

        if is_dir {
            if !matches!(file_name.parse(), Ok(PathType::Directory)) {
                bail!(
                    "subdirectory {:?} has irregular name, invalid collection",
                    e.file_name()
                );
            }
            writeln!(ret, "{file_name}")?;
        } else {
            if !matches!(file_name.parse(), Ok(PathType::File)) {
                bail!(
                    "file {file_name:?} has irregular name, invalid collection",
                );
            }

            writeln!(ret, "{file_name}")?;

            // Print lines
            let file = fs::read_to_string(e.path()).unwrap();

            // XXX: Lots of extra parsing work here that gets thrown out, but
            // we want to know which specific file has errors when the
            // collection isn't deserializable.
            if let Err(_e) = idm::from_str::<Outline>(&file) {
                // XXX: Get IDM errors cleaned up so you can print them here.
                bail!("file {file_name:?} is not valid IDM");
            }

            for line in file.lines() {
                let mut ln = &line[..];
                let mut depth = depth;
                // Turn tab indentation into spaces.
                while ln.starts_with('\t') {
                    depth += 1;
                    ln = &ln[1..];
                }
                for _ in 1..(depth + 1) {
                    write!(ret, "  ")?;
                }
                writeln!(ret, "{ln}")?;
            }
        }
    }

    Ok(idm::from_str(&ret)?)
}

pub fn write_directory<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<()> {
    // Transmute value into an outline.
    let tree: SimpleOutline = idm::transmute(value)?;

    fn construct(
        path: &Path,
        outline: &SimpleOutline,
        output: &mut HashMap<PathBuf, String>,
    ) -> anyhow::Result<()> {
        for ((head,), body) in &outline.0 {
            let head = head.trim();

            match head.parse::<PathType>() {
                Ok(PathType::File) => {
                    // It's a file. Write out the contents.
                    let path = path.join(head);
                    output.insert(path, body.to_string());
                }
                Ok(PathType::Directory) => {
                    // Treat it as directory.
                    let path: PathBuf = path.join(head);
                    // Recurse into body using the new path.
                    construct(&path, body, output)?;
                }
                _ => {
                    bail!("invalid directory-level headline: {head:?}")
                }
            }
        }

        Ok(())
    }

    // Get files from outline.
    let mut output = HashMap::new();
    construct(path.as_ref(), &tree, &mut output)?;

    // Delete non-hidden files not in outline.
    for entry in WalkDir::new(path.as_ref())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && !e
                    .file_name()
                    .to_str()
                    .map(|s| s.starts_with("."))
                    .unwrap_or(false)
        })
    {
        if !output.contains_key(entry.path()) {
            fs::remove_file(entry.path())
                .with_context(|| "failed to remove file")?;
        }
    }

    // Write out outline files.
    for (path, content) in output {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, content.to_string())?;
    }

    Ok(())
}

/// Classify lines of text into file-like, subdirectory-like or neither (not
/// parsed).
enum PathType {
    File,
    Directory,
}

impl FromStr for PathType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        // Whitelists of acceptable chars.
        if regex!(r"^[A-Za-z0-9_-]*(\.[A-Za-z0-9_-]+)+$").is_match(s) {
            Ok(PathType::File)
        } else if regex!(r"^[A-Za-z0-9_-]+$").is_match(s) {
            Ok(PathType::Directory)
        } else {
            bail!("Invalid path string")
        }
    }
}

/// Something looks like a filename if it has a period followed immediately by
/// a non-whitespace character.
fn looks_like_filename(name: impl AsRef<str>) -> bool {
    regex!(r"\.\S").is_match(name.as_ref())
}
