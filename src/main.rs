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
    FindDupes {
        /// Path to the collection.
        collection_path: PathBuf,
    },

    /// Flatten a collection into a single outline.
    Glob {
        /// Path to the collection.
        collection_path: PathBuf,

        /// Output file path, defaults to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Filter items that already exist in the collection out of the input.
    RemoveExisting {
        /// Path to the collection.
        collection_path: PathBuf,

        #[command(flatten)]
        io: IoArgs,
    },
}

use Commands::*;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Columnize(args) => columnize::run(args.try_into()?),
        Cat(args) => todo!(),
        FindDupes { collection_path } => todo!(),
        Glob {
            collection_path,
            output,
        } => todo!(),
        RemoveExisting {
            collection_path,
            io,
        } => todo!(),
    }
}

mod columnize;

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
