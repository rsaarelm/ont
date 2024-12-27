#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! clap = { version = "4", features = ["derive"] }
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use clap::Parser;
use idm_tools::{Collection, Outline};

#[derive(Parser)]
struct Args {
    /// Path to the collection directory.
    collection_path: String,

    /// Show counts for tags.
    #[arg(short = 'f', long, default_value_t = false)]
    histogram: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut collection: Collection<Outline> =
        Collection::load(&args.collection_path).unwrap();

    let mut hist: BTreeMap<String, usize> = BTreeMap::default();

    for (a, sec) in collection.context_iter_mut(BTreeSet::default()) {
        if let Some(tags) = sec.1.get::<BTreeSet<String>>("tags").unwrap() {
            a.extend(tags.iter().cloned());
            for tag in a.iter() {
                *hist.entry(tag.into()).or_default() += 1;
            }
        }
    }

    if args.histogram {
        let mut hist = hist.into_iter().collect::<Vec<_>>();
        hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (tag, n) in &hist {
            println!("{tag:32} {n}");
        }
    } else {
        for tag in hist.keys() {
            println!("{tag}");
        }
    }
    Ok(())
}
