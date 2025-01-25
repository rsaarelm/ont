use std::{io::Read, path::PathBuf};

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

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
    Echo(IoArgs),

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
        Columnize(args) => columnize::run(args),
        Echo(args) => todo!(),
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

    /// Output file path, defaults to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl IoArgs {
    pub fn read(&self) -> Result<String> {
        let mut input = String::new();
        if self.input == PathBuf::from("-") {
            std::io::stdin().read_to_string(&mut input)?;
            Ok(input)
        } else {
            Ok(std::fs::read_to_string(&self.input)?)
        }
    }

    pub fn write(&self, output: &str) -> Result<()> {
        if let Some(outfile) = &self.output {
            std::fs::write(outfile, output)?;
        } else {
            print!("{}", output);
        }
        Ok(())
    }
}
