use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use idm_tools::Outline;

use crate::IoPipe;

pub fn run(collection: PathBuf, io: IoPipe) -> Result<()> {
    let mut outline: Outline = io.read_outline()?;

    let (collection, _) = idm_tools::read_outline(collection)?;

    let mut existing: BTreeSet<String> = BTreeSet::default();

    for s in collection.iter() {
        existing.extend(s.body.uris()?);
    }

    let mut removes = 0;

    for s in outline.iter_mut() {
        for i in (0..s.body.children.len()).rev() {
            let Some(uri) =
                s.body.children[i].body.get::<String>("uri").unwrap()
            else {
                continue;
            };

            if existing.contains(&uri) {
                s.body.children.remove(i);
                removes += 1;
            }
        }
    }

    eprintln!(
        "Removed {} item(s) matching {} already found in collection",
        removes,
        existing.len()
    );

    io.write(&outline)
}
