use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use ont::{Outline, parse};

use crate::IoPipe;

pub fn run(io: IoPipe, collection: PathBuf, strict: bool) -> Result<()> {
    let mut outline: Outline = io.read_outline()?;

    let (collection, _) = ont::read_outline(collection)?;

    let mut existing: BTreeSet<String> = BTreeSet::default();

    for s in collection.iter() {
        for uri in s.body.uris()? {
            if strict {
                existing.insert(uri);
            } else {
                existing.insert(parse::normalized_url(&uri));
            }
        }
    }

    let mut removes = 0;

    for s in outline.iter_mut() {
        for i in (0..s.body.children.len()).rev() {
            let Some(mut uri) =
                s.body.children[i].body.get::<String>("uri").unwrap()
            else {
                continue;
            };

            if !strict {
                uri = parse::normalized_url(&uri);
            }

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
