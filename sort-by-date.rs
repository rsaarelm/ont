#!/usr/bin/env rust-script

//! Print items from stdin sorted by :date values to stdout.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use anyhow::Result;
use idm_tools::Outline;
use std::{
    cmp::Ordering,
    io::{self, Read},
};

fn ord(
    ((_,), a): &((String,), Outline),
    ((_,), b): &((String,), Outline),
) -> Ordering {
    let blank = String::new();
    a.0 .0
        .get("date")
        .unwrap_or(&blank)
        .cmp(b.0 .0.get("date").unwrap_or(&blank))
}

fn main() -> Result<()> {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    let mut outline: Outline = idm::from_str(&stdin)?;

    outline.1.sort_by(ord);

    print!("{}", idm::to_string_styled_like(&stdin, &outline).unwrap());

    Ok(())
}
