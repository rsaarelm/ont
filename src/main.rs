use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod io_pipe;
use io_pipe::IoPipe;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert a list of rows into a list of columns from those rows.
    Columnize(IoArgs),

    /// Parse input into IDM and echo it back, use to find unparseable input
    /// or irregularities that don't survive a roundtrip.
    Cat(IoArgs),

    /// Find duplicate elements in a collection.
    FindDupes(IoArgs),

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

    /// Filter out items with URIs that exist in collection from the input.
    FilterExisting {
        /// Path to existing collection input will be compared against.
        collection: PathBuf,

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
        Columnize(args) => columnize::run(args.try_into()?),
        Cat(args) => {
            let io = IoPipe::try_from(args)?;
            io.write(&io.read_outline()?)
        }
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
    }
}

mod columnize;
mod filter_existing;
mod find_dupes;
mod sort_by;
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
