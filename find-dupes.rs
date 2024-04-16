#!/usr/bin/env rust-script

//! Find object IDs that have multiple instances in the collection.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { path = "/home/rsaarelm/work/idm-tools", version = "*" }
//! lazy-regex = "3"
//! ```

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use idm_tools::{Collection, Outline, Section};
use lazy_regex::regex_captures;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum Id {
    Title(String),
    Uri(String),
}

impl TryFrom<Section> for Id {
    type Error = anyhow::Error;

    fn try_from(
        ((head,), Outline((attrs,), _)): Section,
    ) -> Result<Self> {
        if let Some(title) = wiki_title(&head) {
            Ok(Id::Title(title.into()))
        } else if let Some(uri) = attrs.get("uri") {
            Ok(Id::Uri(uri.into()))
        } else {
            bail!("not an object")
        }
    }
}

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let mut collection: Collection<Outline> = Collection::load(path).unwrap();

    let mut seen: BTreeSet<Id> = BTreeSet::default();

    collection.for_each::<()>(|mut a, s| {
        let Ok(id) = Id::try_from(s) else { return () };
        if seen.contains(&id) {
            println!("{id:?}");
        }
        seen.insert(id);
        ()
    });

    Ok(())
}

fn wiki_title(headline: &str) -> Option<&str> {
    regex_captures!(r"^([A-Z][a-z]+([A-Z][a-z]+|\d+)+)( \*)?$", headline)
        .map(|(_, ret, _, _)| ret)
}
