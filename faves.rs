#!/usr/bin/env rust-script

//! List favorited items from outline from stdin.
//!
//! NB. Some tags are inferred from parent outlines of items, and these tags
//! will no longer be available in `faves` output. In pipeline code, do tag
//! search first, fave filtering second.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use std::io::{self, Read};

use anyhow::Result;
use idm_tools::Outline;

fn main() -> Result<()> {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    let mut outline: Outline = idm::from_str(&stdin)?;
    let style = idm::ser::Indentation::infer(&stdin).unwrap_or_default();

    for ((head,), body) in outline.iter_mut() {
        if head.ends_with(" *") {
            print!("{}", idm::to_string_styled(style, &((head,), body)).unwrap());
        }
    }

    Ok(())
}
