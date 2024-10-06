#!/usr/bin/env rust-script

//! Filter items that contain given tags from stdin input.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use std::{
    collections::BTreeSet,
    io::{self, Read},
};

use anyhow::Result;
use idm_tools::Outline;

fn main() -> Result<()> {
    let search_tags = std::env::args().skip(1).collect::<BTreeSet<_>>();

    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    let mut outline: Outline = idm::from_str(&stdin)?;
    let style = idm::ser::Indentation::infer(&stdin).unwrap_or_default();

    for ref section @ ((_,), Outline((ref attrs,), _)) in outline.iter_mut() {
        let Some(tags) = attrs.get("tags") else {
            continue;
        };

        let Ok(tags): Result<BTreeSet<String>, _> = idm::from_str(&tags) else {
            eprintln!("Invalid tags {tags:?}");
            continue;
        };

        if tags.is_superset(&search_tags) {
            print!(
                "{}",
                idm::to_string_styled(style, &section).unwrap());
        }
    }

    Ok(())
}
