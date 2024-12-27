#!/usr/bin/env rust-script

//! Remove items found in collection from stdin input.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use std::{io::Read, collections::HashSet};

use anyhow::Result;
use idm_tools::{Collection, Outline};

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let mut collection: Collection<Outline> = Collection::load(path)?;

    // Read stdin into an Outline value
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    let mut input: Outline = idm::from_str(&stdin)?;

    // Create a HashSet of all the ":uri" tag values found in collection
    // items.
    let mut existing_uris: HashSet<String> = HashSet::new();
    for (_, outline) in collection.iter_mut() {
        if let Some(uri) = outline.get("uri").unwrap() {
            existing_uris.insert(uri);
        }
    }

    let mut removes = 0;

    // Walk through outline and remove every item that has an ":uri" tag that
    // matches a value in existing_uris.
    for (_, Outline(_, secs)) in input.iter_mut() {
        for i in (0..secs.len()).rev() {
            if let Some(uri) = secs[i].1.get::<String>("uri").unwrap() {
                if existing_uris.contains(&uri) {
                    secs.swap_remove(i);
                    removes += 1;
                }
            }
        }
    }

    print!("{input}");

    eprintln!("Removed {} item(s) matching {} already found in collection", removes, existing_uris.len());

    Ok(())
}
