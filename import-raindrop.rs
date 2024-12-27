#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! anyhow = "1"
//! clap = { version = "4", features = ["derive"] }
//! csv = "1"
//! idm = "0.4"
//! idm-tools = { git = "https://github.com/rsaarelm/idm-tools", version = "*" }
//! serde = { version = "1", features = ["derive"] }
//! ```

// Import bookmarks CSV from raindrop.io into an IDM collection.

use std::collections::BTreeMap;

use anyhow::Result;
use clap::Parser;
use idm_tools::{Outline, Section};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
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
        let mut tags: Vec<String> = b
            .tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        let mut title = b.title.clone();

        // Morph favorite tag.
        if let Some(pos) = tags.iter().position(|t| t == "*") {
            tags.remove(pos);
            title.push_str(" *");
        }

        if b.favorite && !title.ends_with(" *") {
            title.push_str(" *");
        }

        let mut sec = ((title,), Outline::default());
        let o = &mut sec.1;
        o.set("uri", &b.url).unwrap();
        if !tags.is_empty() {
            o.set("tags", &tags).unwrap();
        }
        o.set("added", &b.created).unwrap();

        let text = format!("{}\n{}\n", b.excerpt.trim(), b.note.trim());
        if !text.trim().is_empty() {
            let mut body: Vec<Section> =
                idm::from_str(text.trim_start()).unwrap();
            if !body.is_empty() && body[body.len() - 1].0 .0.is_empty() {
                body.swap_remove(body.len() - 1);
            }
            o.1 = body;
        }

        sec
    }
}

// Read input file as positional clap argument or use stdin if unspecified.
// Specify output file as -o or --output argument, use stdout if unspecified.
#[derive(Parser)]
struct Args {
    /// Input file, use stdin if unspecified.
    input: Option<String>,

    /// Optional output file.
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Read input CSV file into a Vec<Bookmark> list. The first line in the
    // CSV is a list of field names, not data.
    let bookmarks: Vec<Bookmark> = if let Some(input) = args.input {
        let mut rdr = csv::Reader::from_path(input)?;
        rdr.deserialize().collect::<Result<_, _>>()?
    } else {
        let mut rdr = csv::Reader::from_reader(std::io::stdin());
        rdr.deserialize().collect::<Result<_, _>>()?
    };

    let mut folders: BTreeMap<String, Vec<Bookmark>> = BTreeMap::new();

    for b in &bookmarks {
        folders.entry(b.folder.clone()).or_default().push(b.clone());
    }

    for (f, bs) in folders.iter_mut() {
        bs.sort_by_key(|b| b.created.clone());
    }

    // Convert folders into an IDM outline and print it.
    let mut outline = Outline::default();
    for (name, bs) in folders {
        let mut sec = ((name,), Outline::default());
        sec.1 .1 = bs.into_iter().map(|b| b.into()).collect();
        outline.1.push(sec);
    }

    let idm = idm::to_string(&outline)?;

    // If output channel was specified, write to it, otherwise print to
    // stdout.
    if let Some(output) = args.output {
        std::fs::write(output, idm)?;
    } else {
        print!("{}", idm);
    };

    Ok(())
}
