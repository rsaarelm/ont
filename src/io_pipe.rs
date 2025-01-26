use std::{collections::BTreeSet, io::Read, path::PathBuf};

use anyhow::{bail, Result};
use idm::ser::Indentation;
use idm_tools::Outline;

use crate::IoArgs;

/// Structure that abstracts the input and output of subcommands.
///
/// Either can be either a single file or a collection of files in a
/// directory.
pub struct IoPipe {
    source: Source,
    dest: PathBuf,
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
            Source::File { ref content, .. } | Source::Stdin(ref content) => {
                Ok(idm::from_str(content)?)
            }
        }
    }

    pub fn write(&self, output: &Outline) -> Result<()> {
        if self.dest.to_str() == Some("-") {
            print!("{}", idm::to_string_styled(self.style(), output)?);
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

        let source = if value.input.to_str() == Some("-") {
            // Read stdin to string.
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
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

        Ok(IoPipe { source, dest })
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
