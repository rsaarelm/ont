#!/usr/bin/env rust-script

//! Weave outputs of embedded script files into the document.
//!
//! Example use:
//! ```notrust
//! This is an outline file
//! Use filename followed by colon to indicate an embedded script file
//! Have a shebang line as first line to show the script is executable
//! Have a '==' marker immediately below the script to indicate
//!   the script output should be captured and inserted under it
//! demo.py:
//!   #!/usr/bin/env python3
//!   def fib(x): return x if x <= 1 else fib(x-1)+fib(x-2)
//!   print([fib(i) for i in range(10)])
//! ==
//!   [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
//! ```
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

// TODO: Output from script might not be valid IDM if it indents things
// weirdly, check for this and change indentations to NBSPs to correct for
// this

const OUTPUT_MARKER: &str = "==";

const CACHE_SUBDIR: &str = ".idm-weave";

use std::{
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    let text = fs::read_to_string(&args.path)?;
    let input: Outline = idm::from_str(&text)?;

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
    let outputs = run_files(&orders)?;
    insert_outputs(&mut outline, &outputs)?;

    log::info!("Rewriting outline {:?}", args.path);
    // Munge the outline expression so we drop the synthesized toplevel.
    fs::write(
        &args.path,
        idm::to_string_styled_like(&text, &outline.1[0].1)?,
    )?;

    Ok(())
}

fn write_files(
    cache_dir: &Path,
    outline: &mut Outline,
) -> Result<Vec<RunOrder>> {
    let mut ret = Vec::new();

    for (inside_file, ((head,), Outline(_, body))) in
        outline.context_iter_mut(false)
    {
        if *inside_file {
            continue;
        }

        if filename(&head).is_some() {
            // Trying to go down a file, stop here.
            *inside_file = true;
            continue;
        }

        for i in 0..body.len() {
            let head = body[i].0 .0.trim();
            if let Some(filename) = filename(head) {
                let content = body[i].1.to_string();

                let filename = if filename.is_empty() {
                    // Get hash of content for an anonymous snippet.
                    let mut hasher = Sha256::new();
                    hasher.update(content.as_bytes());
                    format!("{:X}", hasher.finalize())
                } else {
                    filename.to_string()
                };

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

    Ok(ret)
}

fn run_files(orders: &[RunOrder]) -> Result<Vec<String>> {
    let mut ret = Vec::new();

    for order in orders {
        log::info!("Executing {:?}", order.path);
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&order.path)
            .stdout(Stdio::piped())
            .spawn()?;

        let mut output = String::new();
        child.stdout.as_mut().unwrap().read_to_string(&mut output)?;
        let exit_status = child.wait()?;

        if exit_status.success() {
            if order.capture_output {
                ret.push(output);
            }
        } else {
            bail!(
                "Script {:?} exited with error code {exit_status}",
                order.path
            );
        }
    }

    Ok(ret)
}

fn insert_outputs(outline: &mut Outline, outputs: &[String]) -> Result<()> {
    // XXX: Repetitive code between this and write_files
    let mut output_idx = 0;

    for (inside_file, ((head,), Outline(_, body))) in
        outline.context_iter_mut(false)
    {
        if *inside_file {
            continue;
        }

        if filename(&head).is_some() {
            // Trying to go down a file, stop here.
            *inside_file = true;
            continue;
        }

        for i in 0..body.len() {
            let head = body[i].0 .0.trim();
            if let Some(_) = filename(head) {
                let content = body[i].1.to_string();
                let capture_output =
                    i < body.len() - 1 && body[i + 1].0 .0 == OUTPUT_MARKER;
                let is_executable = content.starts_with("#!");

                // Rewrite output.
                if is_executable && capture_output {
                    body[i + 1].1 = idm::from_str(&outputs[output_idx])?;
                    output_idx += 1;
                }
            }
        }
    }

    Ok(())
}

fn filename(head: &str) -> Option<&str> {
    let head = head.trim();
    let Some(head) = head.strip_suffix(':') else {
        return None;
    };

    // TODO Filename validation could be more robust. Must be a
    // valid filename that isn't trying to escape containment with
    // ".." or by starting with '/', but it can include
    // subdirectory structure.
    if head.chars().any(|c| c.is_whitespace())
        || head.contains("..")
        || head.starts_with('/')
    {
        return None;
    }

    Some(head)
}

#[derive(Debug)]
struct RunOrder {
    path: PathBuf,
    capture_output: bool,
}
