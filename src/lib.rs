use std::{fs, path::Path};

use anyhow::{Result, bail};

mod collection;
pub use collection::{read_directory, write_directory};

pub mod parse;

mod outline;
use idm::ser::Indentation;
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
