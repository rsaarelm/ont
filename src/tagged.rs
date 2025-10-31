use std::collections::BTreeSet;

use anyhow::Result;
use ont::{parse, Outline};

use crate::IoPipe;

pub fn run(tag_list: Vec<String>, io: IoPipe) -> Result<()> {
    let outline = io.read_outline()?;

    let search_tags = tag_list
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

    let mut output = Outline::default();

    for (state, s) in outline.context_iter(IterState::default()) {
        if state.stop_recursing {
            continue;
        }

        // A WikiTitle headline counts as a wiki-title tag.
        if let Some(title) = s.wiki_title() {
            state.tags.insert(parse::camel_to_kebab(title));
        }

        // Read tags in the section.
        let Ok(tags) = s.body.get::<Vec<String>>("tags") else {
            continue;
        };

        if let Some(tags) = tags {
            state.tags.extend(tags);
        }

        // If we found a section that matches all tags, add it to result.
        if search_tags.is_subset(&state.tags) {
            state.stop_recursing = true;
            output.push(s.clone());
        }
    }

    io.write(&output)
}

#[derive(Clone, Default, Debug)]
struct IterState {
    tags: BTreeSet<String>,
    stop_recursing: bool,
}
