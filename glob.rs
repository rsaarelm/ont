#!/usr/bin/env rust-script

//! Glob files from a collection at given path to single outline to stdout
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! lazy-regex = "3"
//! ```

use anyhow::Result;
use idm_tools::{Collection, Outline};

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let collection: Collection<Outline> = Collection::load(path).unwrap();
    print!(
        "{}",
        idm::to_string_styled(collection.style, &collection.inner).unwrap()
    );

    Ok(())
}
