use std::collections::BTreeSet;

use anyhow::Result;
use ont::{parse, Outline, Section};

use crate::IoPipe;

pub fn run(io: IoPipe, tag_list: Vec<String>, flatten: bool) -> Result<()> {
    let outline = io.read_outline()?;

    if flatten {
        let mut sections = Vec::new();
        collect_matching(&tag_list, BTreeSet::new(), &outline, &mut sections);
        io.write(&Outline::from_iter(sections))
    } else {
        io.write(&prune_outline(&tag_list, BTreeSet::new(), &outline))
    }
}

fn prune_outline(
    search_tags: &[String],
    mut inherited_tags: BTreeSet<String>,
    outline: &Outline,
) -> Outline {
    let mut pruned = Outline::default();
    // Do not copy attributes into the output when we're recursing.
    // Only bring them in from sections that match the predicate.

    // Inherited set gets copied all over the place, but we'll eat the cost, it should mostly be
    // pretty small.
    if let Ok(Some(tags)) = outline.get::<Vec<String>>("tags") {
        inherited_tags.extend(tags);
    }

    for s in &outline.children {
        // Child is a match if it is a tag-bearing thing that matches all the search tags once we
        // include inherited tags.
        let is_match = {
            let tags = s.tags();

            if !tags.is_empty() {
                let mut set = inherited_tags.clone();
                set.extend(tags);
                search_tags.iter().all(|t| set.contains(t))
            } else {
                false
            }
        };

        // Direct matches go in as is.
        if is_match {
            pruned.push(s.clone());
            continue;
        }

        // Otherwise, recurse into children. Remember to add wiki-title as an inherited tag.
        let body = if let Some(title) = s.wiki_title() {
            let mut set = inherited_tags.clone();
            set.insert(parse::camel_to_kebab(title));
            prune_outline(search_tags, set, &s.body)
        } else {
            prune_outline(search_tags, inherited_tags.clone(), &s.body)
        };

        // Keep the section if at least some children survived.
        if !body.children.is_empty() {
            pruned.push(Section::new(s.head.clone(), body));
        }
    }

    pruned
}

fn collect_matching(
    search_tags: &[String],
    mut inherited_tags: BTreeSet<String>,
    outline: &Outline,
    out: &mut Vec<Section>,
) {
    if let Ok(Some(tags)) = outline.get::<Vec<String>>("tags") {
        inherited_tags.extend(tags);
    }

    for s in &outline.children {
        let is_match = {
            let tags = s.tags();

            if !tags.is_empty() {
                let mut set = inherited_tags.clone();
                set.extend(tags);
                search_tags.iter().all(|t| set.contains(t))
            } else {
                false
            }
        };

        if is_match {
            out.push(s.clone());
        }

        if let Some(title) = s.wiki_title() {
            let mut set = inherited_tags.clone();
            set.insert(parse::camel_to_kebab(title));
            collect_matching(search_tags, set, &s.body, out);
        } else {
            collect_matching(search_tags, inherited_tags.clone(), &s.body, out);
        }
    }
}
