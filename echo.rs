#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm = "0.4"
//! idm-tools = { path = "/home/rsaarelm/work/idm-tools", version = "*" }
//! ```

use anyhow::Result;
use idm_tools::{read_outline_directory, write_outline_directory, Outline};

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let (outline, style): (Outline, _) = read_outline_directory(path).unwrap();

    //print!("{}", idm::to_string_styled(style, &outline)?);
    write_outline_directory(path, style, &outline).unwrap();
    Ok(())
}
