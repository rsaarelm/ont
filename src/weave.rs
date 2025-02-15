use std::{
    collections::HashMap,
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{bail, Result};
use base64::prelude::*;
use idm_tools::{Outline, Section};
use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::IoPipe;

const OUTPUT_MARKER: &str = "==";

pub fn run(force: bool, io: IoPipe) -> Result<()> {
    // Synthesize a toplevel section.
    //
    // We need to look at sections next to each other with weave, the output
    // marker will be next to the embedded file marker, so the processing
    // happens at the level of a section's children, not at individual
    // sections. We need the synthesized extra layer so that the toplevel of a
    // file can be processed.
    let mut outline = Outline::new(
        Default::default(),
        vec![Section::new(String::new(), io.read_outline()?)],
    );

    let mut scripts = HashMap::new();

    for (i, (entered_script, s)) in outline.context_iter_mut(false).enumerate()
    {
        if *entered_script || weave_filename(&s.head).is_some() {
            *entered_script = true;
            continue;
        }

        for (j, s) in s.body.children.iter().enumerate() {
            let Ok(script) = Script::try_from(s) else {
                continue;
            };

            // Save at child index + 1 since we'll be matching against the
            // output marker that's just under the script section next.
            scripts.insert((i, j + 1), script);
        }
    }

    // Create a temporary directory to run the scripts in.
    let tempdir = tempfile::tempdir()?;

    log::info!(
        "Found {} weave fragments, writing to {tempdir:?}...",
        scripts.len()
    );

    for (_, file) in scripts.iter_mut() {
        let path = tempdir.as_ref().join(&file.file_path);
        fs::write(&path, &file.text)?;
        if file.can_run {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        file.file_path = path;
    }

    for (i, (entered_script, s)) in outline.context_iter_mut(false).enumerate()
    {
        if *entered_script || weave_filename(&s.head).is_some() {
            *entered_script = true;
            continue;
        }

        for j in 1..s.body.children.len() {
            // Output is requested at this point.
            if s.body.children[j].head != OUTPUT_MARKER {
                continue;
            }

            // And we found a valid script at the immediately preceding
            // section.
            let Some(script) = scripts.get(&(i, j)) else {
                continue;
            };

            if script.can_run && (script.is_changed || force) {
                log::info!("Running script {:?}", script.file_path);

                let mut child = Command::new("sh")
                    .arg("-c")
                    .arg(&script.file_path)
                    .stdout(Stdio::piped())
                    .spawn()?;

                let mut output = String::new();
                child.stdout.as_mut().unwrap().read_to_string(&mut output)?;
                let exit_status = child.wait()?;

                if !exit_status.success() {
                    bail!(
                        "Script {:?} exited with error code {exit_status}",
                        script.file_path
                    );
                }

                // Insert output under the output marker.
                s.body.children[j].body = idm::from_str(&output)?;

                // Update the script's hash.
                s.body.children[j - 1] = Section::from(script);
            }
        }
    }

    // Write the updated outline, dropping out of the synthesized toplevel.
    io.write(&outline.children[0].body)
}

#[derive(Default, Debug)]
struct Script {
    /// True if the source did not have an input hash or if the current text
    /// does not match the input hash.
    is_changed: bool,
    /// True if the script has a shebang line.
    can_run: bool,
    self_hash: String,
    outline_path: String,
    text: String,

    /// Temp file the script was written in goes here.
    file_path: PathBuf,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Metadata {
    /// Hash for the previously executed script itself and for any
    /// possible additional tracked references. The first element is
    /// always the script's own hash. If the input hash is different from
    /// the hash of the current script, we know that the script needs to
    /// be re-run.
    input: Vec<String>,
}

type ScriptInput = ((String,), ((Metadata,), String));

impl From<&Script> for Section {
    fn from(value: &Script) -> Self {
        Section::new(
            format!(">{}", value.outline_path),
            idm::transmute(&(
                (Metadata {
                    input: vec![value.self_hash.clone()],
                },),
                value.text.clone(),
            ))
            .expect("Failed to serialize script"),
        )
    }
}

impl TryFrom<&Section> for Script {
    type Error = ();

    fn try_from(section: &Section) -> std::result::Result<Self, Self::Error> {
        let Ok(((head,), ((metadata,), text))) =
            idm::transmute::<_, ScriptInput>(section)
        else {
            return Err(());
        };

        if text.is_empty() {
            return Err(());
        }

        let Some(outline_path) = weave_filename(&head) else {
            return Err(());
        };

        // Build a base64'd sha256 hash of the filename and the contents.
        let mut hasher = Sha256::new();
        hasher.update(outline_path.as_bytes());
        hasher.update("\n".as_bytes());
        hasher.update(text.as_bytes());
        let self_hash = BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());

        // Runnable scripts start with a shebang line.
        let can_run = text.starts_with("#!");

        // File is considered changed unless it matches the cached hash exactly.
        let is_changed = if metadata.input.is_empty() {
            true
        } else {
            self_hash != metadata.input[0]
        };

        let file_path = PathBuf::from(if outline_path == "-" {
            self_hash.clone()
        } else {
            outline_path.to_owned()
        });

        Ok(Script {
            is_changed,
            can_run,
            self_hash,
            outline_path: outline_path.to_string(),
            text,
            file_path,
        })
    }
}

fn weave_filename(head: &str) -> Option<&str> {
    let head = head.trim().strip_prefix('>')?;

    // XXX Filename validation could be more robust. Must be a
    // valid filename that isn't trying to escape containment with
    // ".." or by starting with '/', but it can include
    // subdirectory structure.
    if head.chars().any(|c| c.is_whitespace())
        || head.contains("..")
        || head.starts_with('/')
    {
        return None;
    }
    if !regex!(r"^[A-Za-z0-9_-][.A-Za-z0-9_/-]*$").is_match(head.as_ref()) {
        return None;
    }

    Some(head)
}
