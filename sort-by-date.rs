#!/usr/bin/env rust-script

//! Print items from stdin sorted by :date values to stdout.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! clap = { version = "4", features = ["derive"] }
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! ```

use anyhow::Result;
use clap::Parser;
use idm_tools::Outline;
use std::{
    cmp::Ordering,
    io::{self, Read},
};

#[derive(Parser)]
struct Args {
    /// Bubble favorited items marked with trailing ` *` to top of list.
    #[arg(short = 'f', long, default_value_t = false)]
    separate_favorites: bool,
}

fn ord(
    sort_faves: bool,
    ((a_head,), a): &((String,), Outline),
    ((b_head,), b): &((String,), Outline),
) -> Ordering {
    let a_head = if sort_faves {
        !a_head.ends_with(" *")
    } else {
        false
    };
    let b_head = if sort_faves {
        !b_head.ends_with(" *")
    } else {
        false
    };

    let blank = String::new();
    let a_date = a.0 .0.get("date").unwrap_or(&blank);
    let b_date = b.0 .0.get("date").unwrap_or(&blank);

    (a_head, a_date).cmp(&(b_head, b_date))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    let mut outline: Outline = idm::from_str(&stdin)?;

    outline.1.sort_by(|a, b| ord(args.separate_favorites, a, b));

    print!("{}", idm::to_string_styled_like(&stdin, &outline).unwrap());

    Ok(())
}
