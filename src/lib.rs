use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use idm::ser::Indentation;
use indexmap::IndexMap;
use lazy_regex::regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

fn read_directory(
    output: &mut String,
    style: &mut Option<Indentation>,
    prefix: &str,
    outline_mode: bool,
    path: impl AsRef<Path>,
) -> Result<()> {
    let mut elts: Vec<(String, PathBuf)> = Vec::new();

    for e in fs::read_dir(path)? {
        let e = e?;
        let path = e.path();
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let file_name = file_name.to_string_lossy();

        if file_name.starts_with('.') {
            log::debug!("read_directory: skipping dotfile {path:?}");
            continue;
        }

        if !is_valid_filename(&file_name) {
            bail!("read_directory: invalid filename {file_name:?}");
        }

        if path.is_dir() {
            if outline_mode {
                elts.push((format!("{file_name}/"), path.into()));
            } else {
                elts.push((file_name.into(), path.into()));
            }
        } else if path.is_file() {
            match path.extension().map(|a| a.to_string_lossy()) {
                Some(e) if e == "idm" => {
                    elts.push((
                        file_name[..file_name.len() - 4].into(),
                        path.into(),
                    ));
                }
                Some(_) => {
                    // Push other extensions in as-is.
                    elts.push((file_name.into(), path.into()));
                }
                None => {
                    // If they don't, output would assume they had .idm
                    // extensions that got stripped and output with them.
                    bail!("read_directory: file {file_name:?} must have an extension");
                }
            }
        } else {
            // Bail on symlinks.
            bail!("read_directory: unhandled file type {path:?}");
        }
    }

    // Sort into order for outline, make sure names that start with colon come
    // first.
    elts.sort_by(|a, b| {
        (!a.0.starts_with(':'), &a.0).cmp(&(b.0.starts_with(':'), &b.0))
    });

    for (head, path) in elts {
        if path.is_dir() {
            writeln!(output, "{prefix}{head}")?;
            // Recurse into subdirectory.
            read_directory(
                output,
                style,
                &format!("{prefix}  "),
                outline_mode,
                path,
            )?;
        } else if path.is_file() {
            let text = fs::read_to_string(&path)?;

            // It's a single line, just put it right after the headword.
            // This is why file names can't have spaces.
            if !text.contains('\n') {
                writeln!(output, "{prefix}{head} {}", text.trim())?;
                continue;
            }

            // Multiple lines, need to work with indentations etc.
            writeln!(output, "{prefix}{head}")?;
            for line in text.lines() {
                if line.trim().is_empty() {
                    writeln!(output)?;
                    continue;
                }
                write!(output, "{prefix}  ")?;

                if line.starts_with(' ') {
                    match style {
                        None => *style = Some(Indentation::Spaces(2)),
                        Some(Indentation::Tabs) => {
                            bail!("read_directory: inconsistent indentation in {path:?}");
                        }
                        _ => {}
                    }
                }
                if line.starts_with('\t') {
                    match style {
                        None => *style = Some(Indentation::Tabs),
                        Some(Indentation::Spaces(_)) => {
                            bail!("read_directory: inconsistent indentation in {path:?}");
                        }
                        _ => {}
                    }
                }

                let mut ln = &line[..];
                // Turn tab indentation into spaces.
                while ln.starts_with('\t') {
                    write!(output, "  ")?;
                    ln = &ln[1..];
                }
                writeln!(output, "{ln}")?;
            }
        } else {
            // Don't know what this is (symlink?), bail out.
            bail!("read_directory: invalid path {path:?}");
        }
    }

    Ok(())
}

/// Read heterogeneous IDM outline directory.
///
/// Subdirectories are renamed to have a slash behind them so they get
/// rewritten as subdirectories when the outline is written out.
pub fn read_outline_directory<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<(T, Indentation)> {
    let mut idm = String::new();
    let mut indentation = None;
    read_directory(&mut idm, &mut indentation, "", true, path)?;

    Ok((idm::from_str(&idm)?, indentation.unwrap_or_default()))
}

/// Read a structured IDM data directory.
///
/// Subdirectory names are read as is, subdirectory structure will not be
/// preserved if the value is written back into directory.
pub fn read_data_directory<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T> {
    let mut idm = String::new();
    let mut indentation = None;
    read_directory(&mut idm, &mut indentation, "", false, path)?;

    Ok(idm::from_str(&idm)?)
}

fn build_files(
    files: &mut BTreeMap<PathBuf, String>,
    path: impl AsRef<Path>,
    style: Indentation,
    data: &Outline,
) -> Result<()> {
    // Attribute block
    for (key, value) in &data.0 .0 {
        if !is_valid_filename(key) {
            bail!("build_files: bad attribute name {key:?}");
        }
        let path = path.as_ref().join(format!(":{key}.idm"));

        // Ensure value has correct indentation.
        let value = if value.contains('\n') {
            // Only do this with values with newlines, since transmuting to
            // outline will probably mess up the no-trailing-newline semantic
            // difference.
            let value: SimpleOutline = idm::transmute(value)?;
            idm::to_string_styled(style, &value)?
        } else {
            // Newline-less values just get pushed in as is.
            value.into()
        };

        files.insert(path, value);
    }

    // Regular contents
    for ((headline,), data) in &data.1 {
        // You get an empty toplevel section when a file ends with an extra
        // newline. Just ignore these.
        if headline.trim().is_empty() {
            continue;
        }

        let name = if headline.ends_with('/') {
            &headline[0..headline.len() - 1]
        } else {
            &headline[..]
        };
        if !is_valid_filename(name) {
            bail!("build_files: bad headline {headline:?}");
        }

        if headline.ends_with('/') {
            // Create a subdirectory.
            build_files(files, &path.as_ref().join(headline), style, data)?;
            continue;
        }

        let file_name = if headline.contains('.') {
            headline.into()
        } else {
            format!("{headline}.idm")
        };

        let path = path.as_ref().join(file_name);
        files.insert(path, idm::to_string_styled(style, data)?);
    }
    Ok(())
}

fn write_directory(
    path: impl AsRef<Path>,
    style: Indentation,
    data: &Outline,
) -> Result<()> {
    // See that we can build all the contents successfully before deleting
    // anything.
    let mut files = BTreeMap::default();
    build_files(&mut files, path.as_ref(), style, data)?;

    // Clean out old stuff.
    for e in fs::read_dir(path.as_ref())? {
        let e = e?;
        let path = e.path();

        let Some(file_name) = path.file_name() else {
            continue;
        };
        let file_name = file_name.to_string_lossy();

        if file_name.starts_with('.') {
            log::debug!("write_directory: skipping dotfile {path:?}");
            continue;
        }

        if path.is_dir() {
            log::debug!("write_directory: Removing existing dir {path:?}");
            fs::remove_dir_all(path)?;
        } else {
            log::debug!("write_directory: Removing existing file {path:?}");
            fs::remove_file(path)?;
        }
    }

    for (path, content) in files {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, content)?;
    }

    Ok(())
}

pub fn write_outline_directory<T: Serialize>(
    path: impl AsRef<Path>,
    style: Indentation,
    value: &T,
) -> Result<()> {
    let tree: Outline = idm::transmute(value)?;

    write_directory(path, style, &tree)
}

fn is_valid_filename(s: impl AsRef<str>) -> bool {
    regex!(r"^:?[A-Za-z0-9_-][.A-Za-z0-9_-]*$").is_match(s.as_ref())
}
