#!/usr/bin/env rust-script

//! Weave outputs of embedded script files into the document.
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! clap = { version = "4", features = ["derive"] }
//! env_logger = "0.11"
//! log = "0.4"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! sha2 = "0.10"
//! ```

const OUTPUT_MARKER: &str = "==";

const CACHE_SUBDIR: &str = ".idm-weave";

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use clap::Parser;
use idm_tools::Outline;
use sha2::{Digest, Sha256};

#[derive(Parser)]
struct Args {
    /// Clear cached computation results.
    #[arg(long, default_value_t = false)]
    clear_cache: bool,

    path: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    if !args.path.is_file() {
        bail!("Target {:?} is not a valid file", args.path);
    }

    // Read and parse file.
    let input: Outline = idm::from_str(&fs::read_to_string(&args.path)?)?;

    // Generate cache subdirectory.
    let working_dir = args.path.parent().expect("Failed to get working dir");
    let cache_dir = working_dir
        .join(CACHE_SUBDIR)
        .join(args.path.file_name().expect("Bad path"));

    if args.clear_cache {
        log::info!("Clearing existing cache at {cache_dir:?}");
        let _ = fs::remove_dir_all(&cache_dir);
    }

    fs::create_dir_all(&cache_dir)?;
    log::info!("Created cache dir {cache_dir:?}");

    // Synthesize a toplevel section.
    //
    // We need to look at sections next to each other with weave, the output
    // marker will be next to the embedded file marker, so the processing
    // happens at the level of a section's children, not at individual
    // sections. We need the synthesized extra layer so that the toplevel of a
    // file can be processed.
    let mut outline = Outline(
        Default::default(),
        vec![((args.path.to_string_lossy().to_string(),), input)],
    );

    let orders = write_files(&cache_dir, &mut outline)?;

    Ok(())
}

fn write_files(
    cache_dir: &Path,
    outline: &mut Outline,
) -> Result<Vec<RunOrder>> {
    let mut ret = Vec::new();

    for (scan_for_files, (_, Outline(_, body))) in
        outline.context_iter_mut(true)
    {
        if !*scan_for_files {
            continue;
        }

        for i in 0..body.len() {
            let head = body[i].0 .0.trim();
            if let Some(head) = head.strip_suffix(':') {
                // TODO Filename validation could be more robust. Must be a
                // valid filename that isn't trying to escape containment with
                // ".." or by starting with '/', but it can include
                // subdirectory structure.
                if !head.chars().any(|c| c.is_whitespace())
                    && !head.contains("..")
                    && !head.starts_with('/')
                {
                    let content = body[i].1.to_string();

                    let filename = if head.is_empty() {
                        // Get hash of content for an anonymous snippet.
                        let mut hasher = Sha256::new();
                        hasher.update(content.as_bytes());
                        format!("{:X}", hasher.finalize())
                    } else {
                        head.to_owned()
                    };

                    // Use context parameter to stop scanning when we've detected
                    // a file.
                    *scan_for_files = false;

                    let output_path = cache_dir.join(filename);

                    fs::write(&output_path, &content)?;
                    let capture_output =
                        i < body.len() - 1 && body[i + 1].0 .0 == OUTPUT_MARKER;

                    let is_executable = content.starts_with("#!");

                    if is_executable {
                        fs::set_permissions(
                            &output_path,
                            fs::Permissions::from_mode(0o700),
                        )?;

                        ret.push(RunOrder {
                            path: output_path.clone(),
                            capture_output,
                        });
                    }

                    log::info!(
                        "Wrote {output_path:?}{}{}",
                        if is_executable { " (x)" } else { "" },
                        if capture_output { " (cap)" } else { "" }
                    );
                }
            }
        }
    }

    Ok(ret)
}

struct RunOrder {
    path: PathBuf,
    capture_output: bool,
}
