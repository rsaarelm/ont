use std::{fs, path::Path};

use anyhow::Result;

mod collection;
pub use collection::{read_directory, write_directory};

mod outline;
pub use outline::{Outline, Section, SimpleOutline, SimpleSection};

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
