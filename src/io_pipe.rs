use std::{collections::BTreeSet, io::Read, path::PathBuf};

use anyhow::{bail, Result};
use idm::ser::Indentation;
use idm_tools::{parse, Outline};

use crate::IoArgs;

/// Structure that abstracts the input and output of subcommands.
///
/// Either can be either a single file or a collection of files in a
/// directory.
pub struct IoPipe {
    source: Source,
    dest: PathBuf,

    /// Indentation prefix to remove/add when piping fragments from the middle
    /// of a file.
    stdin_prefix: String,
}

impl IoPipe {
    pub fn read_text(&self) -> Result<String> {
        match self.source {
            Source::Stdin(ref content) => Ok(content.clone()),
            Source::File { ref content, .. } => Ok(content.clone()),
            Source::Collection {
                ref outline, style, ..
            } => Ok(idm::to_string_styled(style, outline)?),
        }
    }

    pub fn read_outline(&self) -> Result<Outline> {
        match self.source {
            Source::Collection { ref outline, .. } => Ok(outline.clone()),
            Source::File { ref content, .. } => Ok(idm::from_str(content)?),
            Source::Stdin(ref content) => {
                if self.stdin_prefix.is_empty() {
                    Ok(idm::from_str(content)?)
                } else {
                    let mut stripped = String::new();
                    for line in content.lines() {
                        if line.trim().is_empty() {
                            stripped.push('\n');
                            continue;
                        }

                        let Some(line) = line.strip_prefix(&self.stdin_prefix)
                        else {
                            // We can hit this if the input is mixing tabs and
                            // spaces.
                            bail!("read_outline: Shared indent failure")
                        };
                        stripped.push_str(line);
                        stripped.push('\n');
                    }
                    Ok(idm::from_str(&stripped)?)
                }
            }
        }
    }

    pub fn write_text(&self, output: impl AsRef<str>) -> Result<()> {
        if self.dest.to_str() == Some("-") {
            print!("{}", output.as_ref());
        } else if self.dest.is_dir() {
            bail!("Cannot write text to a directory");
        } else {
            std::fs::write(&self.dest, output.as_ref())?;
        }
        Ok(())
    }

    pub fn write(&self, output: &Outline) -> Result<()> {
        if self.dest.to_str() == Some("-") {
            let s = idm::to_string_styled(self.style(), output)?;
            if self.stdin_prefix.is_empty() {
                print!("{s}");
            } else {
                // Reintroduce the stdin prefix when printing back to stdout.
                for line in s.lines() {
                    if line.trim().is_empty() {
                        println!();
                    } else {
                        println!("{}{line}", self.stdin_prefix);
                    }
                }
            }
        } else if self.dest.is_dir() {
            let files_written =
                idm_tools::write_directory(&self.dest, self.style(), output)?;
            if let (Some(root), true) = (self.path(), self.is_in_place()) {
                // Remove files that were initially read but were not written
                // in output when rewriting a collection in place.
                let original_files = match &self.source {
                    Source::Collection { files, .. } => files.clone(),
                    _ => Default::default(),
                };
                for file in original_files.difference(&files_written) {
                    idm_tools::tidy_delete(root, file)?;
                }
            }
        } else {
            std::fs::write(
                &self.dest,
                idm::to_string_styled(self.style(), output)?,
            )?;
        }
        Ok(())
    }

    fn is_in_place(&self) -> bool {
        match &self.source {
            Source::File { path, .. } => path == &self.dest,
            Source::Collection { path, .. } => path == &self.dest,
            Source::Stdin(_) => false,
        }
    }

    fn style(&self) -> Indentation {
        match &self.source {
            Source::Collection { style, .. } => *style,
            Source::Stdin(content) | Source::File { content, .. } => {
                Indentation::infer(content).unwrap_or_default()
            }
        }
    }

    fn path(&self) -> Option<&PathBuf> {
        match &self.source {
            Source::File { path, .. } | Source::Collection { path, .. } => {
                Some(path)
            }
            Source::Stdin(_) => None,
        }
    }
}

impl TryFrom<IoArgs> for IoPipe {
    type Error = anyhow::Error;

    fn try_from(value: IoArgs) -> Result<Self> {
        if value.in_place && value.input.to_str() == Some("-") {
            anyhow::bail!("Cannot use -i with standard input");
        }

        let mut stdin_prefix = String::new();

        let source = if value.input.to_str() == Some("-") {
            // Read stdin to string.
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;

            // See if all lines are indented by a common amount.
            let nonempty_lines: Vec<&str> =
                input.lines().filter(|l| !l.trim().is_empty()).collect();
            let mut shared_indent = parse::indentation(
                nonempty_lines.iter().copied().next().unwrap_or(""),
            );
            for &line in &nonempty_lines {
                let prefix = parse::indentation(line);
                if prefix.len() < shared_indent.len() {
                    shared_indent = prefix;
                }
            }
            stdin_prefix = shared_indent.to_owned();

            Source::Stdin(input)
        } else if value.input.is_dir() {
            let (outline, style, files) =
                idm_tools::read_directory(&value.input)?;
            Source::Collection {
                path: value.input.clone(),
                files,
                style,
                outline,
            }
        } else if value.input.is_file() {
            let content = std::fs::read_to_string(&value.input)?;
            Source::File {
                path: value.input.clone(),
                content,
            }
        } else {
            bail!("Input is not a file or a directory");
        };

        let dest = match &value.output {
            None if value.in_place => value.input,
            Some(_) if value.in_place => {
                bail!("Cannot use -i with output file");
            }
            None => PathBuf::from("-"),
            Some(x) => x.clone(),
        };

        Ok(IoPipe {
            source,
            dest,
            stdin_prefix,
        })
    }
}

enum Source {
    Stdin(String),
    File {
        path: PathBuf,
        content: String,
    },
    Collection {
        path: PathBuf,
        files: BTreeSet<PathBuf>,
        style: Indentation,
        outline: Outline,
    },
}
