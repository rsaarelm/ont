use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod io_pipe;
use ont::Outline;
use io_pipe::IoPipe;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all links in notes
    AllLinks(IoArgs),

    /// Parse input into IDM and echo it back, use to find unparseable input
    /// or irregularities that don't survive a roundtrip.
    Cat(IoArgs),

    /// Convert a list of rows into a list of columns from those rows.
    Columnize(IoArgs),

    /// Filter out items with URIs that exist in collection from the input.
    FilterExisting {
        /// Path to existing collection input will be compared against.
        collection: PathBuf,

        #[command(flatten)]
        io: IoArgs,
    },

    /// Find duplicate elements in a collection.
    FindDupes(IoArgs),

    /// Import bookmarks from Raindrop.io's CSV export.
    ImportRaindrop(IoArgs),

    /// Look for suspiciously close tags and mark them as errors.
    LintTags(IoArgs),

    /// List all tags in a collection.
    ListTags(IoArgs),

    /// Rewrite tags in a collection based on a replacement list.
    ReplaceTags {
        /// Tag replacement list, a file with lines with format `old1-tag
        /// new1-tag` or `old2-tag new2-tag new3-tag ...`. The old tags will
        /// be removed and all the new tags will be added. Lines with just a
        /// single tag name in the replacements file do nothing.
        replacements: PathBuf,

        #[command(flatten)]
        io: IoArgs,
    },

    /// Filter items that already exist in the collection out of the input.
    SortBy {
        /// Field to sort lexically by.
        #[arg(long, default_value = "date")]
        sort_field: String,

        /// Bubble favorited items (marked with trailing ` *`) to top of list.
        #[arg(short = 'f', long, default_value = "false")]
        separate_favorites: bool,

        #[command(flatten)]
        io: IoArgs,
    },

    /// Find items with specific tags.
    Tagged {
        // A positional input argument is mandatory for this command so we
        // spell out a variant of IoArgs.
        /// Input file path, use '-' for stdin.
        #[arg(required = true)]
        input: PathBuf,

        /// List of tags that must be present in items returned.
        #[arg(required = true)]
        tag_list: Vec<String>,

        /// Output file path, defaults to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Format table columns.
    Tf {
        /// Treat every column as left-aligned textual data, don't try to
        /// detect columns of numeric data.
        #[arg(long)]
        no_number_parsing: bool,

        /// How many columns to align, if set to 0, align every column
        /// shared by all nonempty rows.
        #[arg(long, default_value = "0")]
        num_columns: usize,

        #[command(flatten)]
        io: IoArgs,
    },

    /// Weave outputs of embedded scripts into file.
    Weave {
        /// Ignore cache annotations and re-run all scripts.
        #[arg(long)]
        force: bool,

        #[command(flatten)]
        io: IoArgs,
    },
}

use Commands::*;

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        AllLinks(args) => {
            use ont::Outline;
            let mut seen: std::collections::HashSet<String> =
                Default::default();

            let io = IoPipe::try_from(args)?;
            let mut out = Outline::default();
            for sec in io.read_outline()?.iter() {
                if let Ok(Some(uri)) = sec.body.get::<String>("uri") {
                    if seen.contains(&uri) {
                        continue;
                    }
                    seen.insert(uri.clone());
                    if uri.starts_with("http") {
                        let mut sec = sec.clone();
                        // Only take the metadata.
                        sec.body.children.clear();
                        out.push(sec);
                    }
                }
            }

            io.write(&out)
        }
        Cat(args) => {
            let io = IoPipe::try_from(args)?;
            io.write(&io.read_outline()?)
        }
        Columnize(args) => columnize::run(args.try_into()?),
        FindDupes(args) => find_dupes::run(args.try_into()?),
        SortBy {
            sort_field,
            separate_favorites,
            io,
        } => sort_by::run(io.try_into()?, sort_field, separate_favorites),

        FilterExisting { collection, io } => {
            filter_existing::run(collection, io.try_into()?)
        }

        Weave { force, io } => weave::run(force, io.try_into()?),

        Tagged {
            input,
            tag_list,
            output,
        } => {
            let io = IoArgs {
                input,
                in_place: false,
                output,
            };
            tagged::run(tag_list, io.try_into()?)
        }

        ImportRaindrop(args) => raindrop::run(args.try_into()?),

        ReplaceTags { replacements, io } => {
            replace_tags::run(replacements, io.try_into()?)
        }

        LintTags(args) => {
            let io = IoPipe::try_from(args)?;
            let outline = io.read_outline()?;
            let tags = get_tags(&outline);

            // Collect sets of suspiciously similar tags.
            let mut typos: HashMap<String, Vec<String>> = HashMap::new();
            for (i, a) in tags.iter().enumerate() {
                for b in tags.iter().skip(i + 1) {
                    // It's useless for very short tags.
                    if a.len() <= 4 || b.len() <= 4 {
                        continue;
                    }
                    let distance = levenshtein::levenshtein(a, b);
                    let threshold = match a.len() + b.len() {
                        x if x < 16 => 1,
                        _ => 2,
                    };
                    if distance <= threshold && distance > 0 {
                        typos.entry(a.clone()).or_default().push(b.clone());
                        typos.entry(b.clone()).or_default().push(a.clone());
                    }
                }
            }

            // Deduplicate.
            let typos: BTreeSet<Vec<String>> = typos
                .into_iter()
                .map(|(k, v)| {
                    let mut v = v;
                    v.push(k);
                    v.sort();
                    v
                })
                .collect();

            // Sort by longest (most likely to be an actual typo) last.
            let mut typos: Vec<Vec<String>> = typos.into_iter().collect();
            typos.sort_by_key(|v| v[0].len());

            for seq in &typos {
                for w in seq {
                    print!("{} ", w);
                }
                println!();
            }

            Ok(())
        }

        ListTags(args) => {
            use std::fmt::Write;

            let io = IoPipe::try_from(args)?;
            let outline = io.read_outline()?;
            let tags = get_tags(&outline);
            let mut out = String::new();
            for tag in tags {
                writeln!(out, "{}", tag)?;
            }
            io.write_text(&out)
        }

        Tf {
            no_number_parsing,
            num_columns,
            io,
        } => tf::run(no_number_parsing, num_columns, io.try_into()?),
    }
}

mod columnize;
mod filter_existing;
mod find_dupes;
mod raindrop;
mod replace_tags;
mod sort_by;
mod tagged;
mod tf;
mod weave;

/// Standard input/output specification for subcommands.
///
/// By default the subcommand reads from stdin and writes to stdout, this
/// allows pointing to files instead.
#[derive(Debug, Args, Clone)]
pub struct IoArgs {
    /// Input file path, defaults to stdin.
    #[arg(default_value = "-")]
    input: PathBuf,

    /// Whether to modify a file or collection in-place with the tool.
    ///
    /// It's an error to set this option if reading input from stdin.
    #[arg(short, long, default_value = "false")]
    in_place: bool,

    /// Output file path, defaults to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
    // XXX Using Option here instead of just setting it to default to "-" so
    // that we can differentiate between the user explicitly asking for stdout
    // output or just writing minimal calls that might blast a whole
    // collection to stdout. Haven't bothered to implement that yet though.
}

fn get_tags(outline: &Outline) -> BTreeSet<String> {
    outline
        .iter()
        .flat_map(|s| {
            s.body
                .get::<Vec<String>>("tags")
                .unwrap_or_default()
                .unwrap_or_default()
        })
        .collect()
}
