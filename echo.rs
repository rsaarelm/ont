#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! idm-tools = { path = "/home/rsaarelm/work/idm-tools", version = "*" }
//! ```

use anyhow::Result;
use idm_tools::{Outline, read_directory, write_directory};

fn main() -> Result<()> {
    let path = &std::env::args().collect::<Vec<_>>()[1];
    let outline: Outline = read_directory(path).unwrap();

    //write_directory(path, &outline).unwrap();
    Ok(())
}
