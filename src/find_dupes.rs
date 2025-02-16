use std::{collections::BTreeMap, fmt};

use anyhow::{bail, Result};
use idm_tools::{Outline, Section};

use crate::IoPipe;

pub fn run(io: IoPipe) -> Result<()> {
    let mut outline: Outline = io.read_outline()?;

    let mut items: BTreeMap<Id, Vec<Section>> = BTreeMap::default();

    for s in outline.iter_mut() {
        let Ok(id) = Id::try_from(s.clone()) else {
            continue;
        };

        items.entry(id).or_default().push(s.clone());

        // Some bookmarks can be a sequence of URLs, when we encounter one of
        // those, add the rest into the seen set so we can be on the lookout
        // for those showing up individually elsewhere.
        if let Ok(Some(seq)) = s.body.get::<Vec<String>>("sequence") {
            for uri in seq {
                let id = Id::Uri(uri.to_string());
                items.entry(id).or_default().push(s.clone());
            }
        }
    }

    let mut list = Outline::default();
    for (id, sections) in items {
        if sections.len() > 1 {
            let mut dupe = Section::new(id.to_string(), Outline::default());
            for s in sections {
                dupe.body.children.push(s);
            }

            list.children.push(dupe);
        }
    }

    print!("{list}");

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum Id {
    Title(String),
    Uri(String),
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Title(title) => write!(f, "{title}"),
            Id::Uri(uri) => write!(f, "{uri}"),
        }
    }
}

impl TryFrom<Section> for Id {
    type Error = anyhow::Error;

    fn try_from(section: Section) -> Result<Self> {
        if let Some(title) = section.wiki_title() {
            Ok(Id::Title(title.into()))
        } else if let Some(uri) = section.body.attrs.get("uri") {
            // Normalize http/https differences.
            if let Some(suffix) = uri.strip_prefix("http:") {
                Ok(Id::Uri(format!("https:{suffix}")))
            } else {
                Ok(Id::Uri(uri.into()))
            }
        } else {
            bail!("not an object")
        }
    }
}
