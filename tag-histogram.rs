#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { path = "/home/rsaarelm/work/idm-tools", version = "*" }
//! ```

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use idm_tools::{Collection, Outline};

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let mut collection: Collection<Outline> = Collection::load(path).unwrap();

    let mut hist: BTreeMap<String, usize> = BTreeMap::default();

    collection.for_each::<BTreeSet<String>>(
        |mut a, sec| {
            if let Some(tags) = sec.1.get::<BTreeSet<String>>("tags").unwrap() {
                a.extend(tags.iter().cloned());
                for tag in &a {
                    *hist.entry(tag.into()).or_default() += 1;
                }
            }
            a
        });

    for (tag, n) in &hist {
        println!("{tag:32} {n}");
    }
    Ok(())
}

