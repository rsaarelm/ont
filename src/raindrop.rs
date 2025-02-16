use anyhow::Result;
use idm_tools::{Outline, Section};
use serde::Deserialize;

use crate::IoPipe;

pub fn run(io: IoPipe) -> Result<()> {
    let content = io.read_text()?;

    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let bookmarks: Vec<Bookmark> =
        rdr.deserialize().collect::<Result<_, _>>()?;
    let outline: Outline = bookmarks.into_iter().map(Section::from).collect();

    io.write(&outline)
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
struct Bookmark {
    id: String,
    title: String,
    note: String,
    excerpt: String,
    url: String,
    folder: String,
    tags: String,
    created: String,
    cover: String,
    highlights: String,
    favorite: bool,
}

impl From<Bookmark> for Section {
    fn from(b: Bookmark) -> Self {
        let tags: Vec<String> = b
            .tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        let mut title = b.title.clone();

        // Convert raindrop's favorite flag into our convention.
        if b.favorite && !title.ends_with(" *") {
            title.push_str(" *");
        }

        let mut sec = Section::new(title, Default::default());

        let text = format!("{}\n{}\n", b.excerpt.trim(), b.note.trim());
        let mut outline: Outline = idm::from_str(text.trim()).unwrap();

        // Hacky stuff to push the default attributes up top of the list
        // before the freeform ones pulled in from text.
        outline
            .attrs
            .insert_before(0, "tags".to_owned(), tags.join(" "));
        outline
            .attrs
            .insert_before(0, "added".to_owned(), b.created.clone());
        outline
            .attrs
            .insert_before(0, "uri".to_owned(), b.url.clone());

        sec.body = outline;

        sec
    }
}
