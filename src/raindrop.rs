use std::collections::BTreeMap;

use anyhow::Result;
use ont::{web_url, Outline, Section};
use serde::{Deserialize, Serialize};

use crate::IoPipe;

pub fn import(io: IoPipe) -> Result<()> {
    let content = io.read_text()?;

    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut bookmarks: Vec<Bookmark> =
        rdr.deserialize().collect::<Result<_, _>>()?;

    // Put them in ascending order.
    bookmarks.sort_by_key(|b| b.created.clone());

    // Separate bookmarks by folder.
    let mut folders: BTreeMap<String, Outline> = Default::default();
    for b in bookmarks {
        folders
            .entry(b.folder.clone())
            .or_default()
            .children
            .push(Section::from(b));
    }

    let outline: Outline = folders
        .into_iter()
        .map(|(folder, o)| Section::new(folder, o))
        .collect();

    io.write(&outline)
}

pub fn export(io: IoPipe, folder: impl AsRef<str>) -> Result<()> {
    let folder = folder.as_ref();
    let outline: Outline = io.read_outline()?;

    let links = outline
        .iter()
        .filter(|s| web_url(s).is_some())
        .collect::<Vec<_>>();

    // Use io.dest to determine if we're writing to stdout ("-") or a file.
    // It might also be a directroy, in which case throw an error.

    let mut buf = Vec::new();
    {
        let mut wtr = csv::Writer::from_writer(&mut buf);
        wtr.write_record(&[
            "url", "folder", "title", "tags", "created", "note",
        ])?;

        for link in links {
            let mut r = ExportBookmark::from(link.clone());
            r.folder = folder.to_string();
            wtr.write_record(&[
                r.url, r.folder, r.title, r.tags, r.created, r.note,
            ])?;
        }

        wtr.flush()?;
    }

    io.write_text(std::str::from_utf8(&buf)?)?;

    Ok(())
}

#[derive(Clone, Debug, Default, Deserialize)]
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

        // Final hack, if the map contains ":title" key, move it to first position.
        // This is used for wiki-style references whose headline is a WikiWord and the actual title
        // is in the attributes.
        if outline.attrs.contains_key("title") {
            // This moves the existing "title" entry to index 0.
            outline.attrs.shift_insert(
                0,
                "title".to_string(),
                outline.attrs["title"].clone(),
            );
        }

        sec.body = outline;

        sec
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct ExportBookmark {
    url: String,
    folder: String,
    title: String,
    // Format tag1, tag2, CSV library should quote it when serializing...
    tags: String,
    /// ISO 8601 unix timestamp
    created: String,
    note: String,
}

impl From<Section> for ExportBookmark {
    fn from(value: Section) -> Self {
        // This is a bit tricky, we want to convert the matching attributes to Bookmark fields, then
        // insert the remaining attributes and body as IDM string into the 'note' field.

        let url = web_url(&value).unwrap_or_default();
        let (title, mut body) = (value.head, value.body);
        let tags = body
            .get::<Vec<String>>("tags")
            .unwrap()
            .map(|v| v.join(", "))
            .unwrap_or_default();
        let created = body.get::<String>("added").unwrap().unwrap_or_default();

        // Remove the handled keys.
        body.attrs.shift_remove("tags");
        body.attrs.shift_remove("added");
        body.attrs.shift_remove("uri");

        // Turn the rest fo the body into an IDM string.
        let note = idm::to_string(&body)
            .unwrap_or_default()
            .trim_end()
            .to_string();

        ExportBookmark {
            url,
            title,
            tags,
            created,
            note,
            ..Default::default()
        }
    }
}
