use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use idm::ser::Indentation;
use lazy_regex::regex;

use crate::{Outline, SimpleOutline};

pub fn read_directory(
    path: impl AsRef<Path>,
) -> Result<(Outline, Indentation, BTreeSet<PathBuf>)> {
    fn read(
        output: &mut String,
        paths: &mut BTreeSet<PathBuf>,
        style: &mut Option<Indentation>,
        prefix: &str,
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
                elts.push((format!("{file_name}/"), path));
            } else if path.is_file() {
                match path.extension().map(|a| a.to_string_lossy()) {
                    Some(e) if e == "idm" => {
                        // Strip ".idm" extensions, .idm files can be used as
                        // stand-ins for struct fields in an outline that gets
                        // parsed as a data structure.
                        elts.push((
                            file_name[..file_name.len() - 4].into(),
                            path,
                        ));
                    }
                    _ => {
                        // Push other file names as is and append a colon.
                        elts.push((format!("{file_name}:"), path));
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
                read(output, paths, style, &format!("{prefix}  "), path)?;
            } else if path.is_file() {
                paths.insert(path.clone());

                // Read and insert file contents.
                let Ok(text) = fs::read_to_string(&path) else {
                    eprintln!(
                        "read_directory: Skipping non-UTF-8 file {path:?}"
                    );
                    continue;
                };

                // Check that the contents can be IDM-ed in principle.
                if idm::from_str::<Outline>(&text).is_err() {
                    eprintln!(
                        "read_directory: Skipping non-IDM-able file {path:?}"
                    );
                    continue;
                }

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

                    // Figure out style. The entire collection must use the
                    // same style. We set it the first time we see
                    // indentation.
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

                    let mut ln = line;
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

    let mut buf = String::new();
    let mut style = None;
    let mut paths = BTreeSet::default();

    read(&mut buf, &mut paths, &mut style, "", path)?;

    Ok((idm::from_str(&buf)?, style.unwrap_or_default(), paths))
}

pub fn write_directory(
    path: impl AsRef<Path>,
    style: Indentation,
    data: &Outline,
) -> Result<BTreeSet<PathBuf>> {
    // See that we can build all the contents successfully before deleting
    // anything.
    let mut files = BTreeMap::default();
    build_files(&mut files, path.as_ref(), style, data)?;

    let paths = files.keys().cloned().collect();

    for (path, content) in files {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, content)?;
    }

    Ok(paths)
}

fn build_files(
    files: &mut BTreeMap<PathBuf, String>,
    path: impl AsRef<Path>,
    style: Indentation,
    data: &Outline,
) -> Result<()> {
    // Attribute block
    for (key, value) in &data.attrs {
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
    for section in &data.children {
        // You get an empty toplevel section when a file ends with an extra
        // newline. Just ignore these.
        if section.head.trim().is_empty() {
            continue;
        }

        let mut is_directory = false;

        let file_name = if let Some(name) = section.head.strip_suffix('/') {
            is_directory = true;
            name.to_owned()
        } else if let Some(name) = section.head.strip_suffix(':') {
            // File name ends in colon, it's some random non-IDM file.
            name.to_owned()
        } else {
            // Implicit filename, assume an .idm extension.
            format!("{}.idm", section.head)
        };

        if !is_valid_filename(&file_name) {
            bail!("build_files: bad headline {:?}", section.head);
        }

        if is_directory {
            // Create a subdirectory.
            build_files(files, path.as_ref().join(&section.head), style, &section.body)?;
            continue;
        }

        let path = path.as_ref().join(file_name);
        files.insert(path, idm::to_string_styled(style, &section.body)?);
    }
    Ok(())
}

fn is_valid_filename(s: impl AsRef<str>) -> bool {
    regex!(r"^:?[A-Za-z0-9_-][.A-Za-z0-9_-]*$").is_match(s.as_ref())
}
