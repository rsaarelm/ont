#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! clap = { version = "4", features = ["derive"] }
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

// Rewrite file format:
//
//     original-tag    replacement-tag1 replacement-tag2
//     other-original  other-replacement1 other-replacement2
//     unaffected-tag
//
// The first tag is the tag being replaced, the rest are the tags that
// repalace it. Lines with just the single tag do nothing. You can start with
// a dump of all the tags from the collection and write replacement sets to
// only those tags that you want to replace while leaving the rest in place.

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use idm_tools::{Collection, Outline};

// Clap arguments that take collection directory in --collection flag and the
// tag replacement list as a positional argument.
#[derive(Parser)]
struct Args {
    /// Path to the collection directory, use current directory if not given.
    #[arg(short = 'c', long)]
    collection: Option<PathBuf>,

    /// File with tag replacements.
    replacements: PathBuf,
}

// Replacements file is desirialized with IDM.
fn main() -> Result<()> {
    let args = Args::parse();

    let mut collection: Collection<Outline> =
        Collection::load(&args.collection.unwrap_or_else(|| ".".into()))?;

    let replacements: HashMap<String, Vec<String>> =
        idm::from_str(&std::fs::read_to_string(&args.replacements)?)?;

    let mut item_count = 0;
    for sec in collection.iter_mut() {
        if let Some(mut tags) = sec.1.get::<Vec<String>>("tags").unwrap() {
            for i in (0..tags.len()).rev() {
                if let Some(replacements) = replacements.get(&tags[i]) {
                    // Empty items in replacement list are no-ops, you can't
                    // remove tags without replacement with this.
                    if replacements.is_empty() {
                        continue;
                    }

                    item_count += 1;

                    // Replace the original tag with the first replacement
                    // one.
                    if !replacements.contains(&tags[i]) {
                        tags[i] = replacements[0].clone();
                    }

                    // Append the remaining replacement tags unless they're
                    // already found in the set.
                    for t in replacements.iter().skip(1) {
                        if !tags.contains(t) {
                            tags.push(t.clone());
                        }
                    }
                }
            }

            // Replace the tags.
            sec.1.set("tags", &tags).unwrap();
        }
    }

    collection.save().unwrap();
    eprintln!("Replaced tags in {item_count} item(s).");
    Ok(())
}
