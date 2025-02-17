use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;

use crate::IoPipe;

pub fn run(replacements: PathBuf, io: IoPipe) -> Result<()> {
    let mut outline = io.read_outline()?;

    let replacements: HashMap<String, Vec<String>> =
        idm::from_str(&std::fs::read_to_string(&replacements)?)?;

    let mut item_count = 0;
    for s in outline.iter_mut() {
        if let Some(mut tags) = s.body.get_mut::<Vec<String>>("tags").unwrap() {
            for i in (0..tags.len()).rev() {
                if let Some(replacements) = replacements.get(&tags[i]) {
                    // Empty items in replacement list are no-ops, you can't
                    // remove tags without replacement with this.
                    if replacements.is_empty() {
                        continue;
                    }

                    item_count += 1;

                    // Replace the original tag with the first replacement
                    // one (unless the tag list already contains the
                    // replacement).
                    if !replacements.contains(&tags[i]) {
                        if !tags.contains(&replacements[0]) {
                            tags[i] = replacements[0].clone();
                        } else {
                            tags.swap_remove(i);
                        }
                    }

                    // Append the remaining replacement tags unless they're
                    // already found in the set.
                    for t in replacements.iter().skip(1) {
                        if !tags.contains(t) {
                            tags.push(t.clone());
                        }
                    }
                }
            }
        }
    }

    eprintln!("Replaced {item_count} tags");

    io.write(&outline)
}
