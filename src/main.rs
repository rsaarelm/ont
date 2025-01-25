use std::{io::Read, path::PathBuf};

use anyhow::{bail, Result};
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

use idm_tools::{Collection, Outline};
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

    /// Whether to modify a file or collection in-place with the tool.
    ///
    /// It's an error to set this option if reading input from stdin.
    #[arg(short, long, default_value = "false")]
    in_place: bool,

    /// Output file path, defaults to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

// Okay, this gets into hairy weird hack territory.
//
// input can be stdin, file (file path) or collection (directory path).
// All can be read into a string or an outline value.
// Only a collection can be read into a collection value.
// (Or maybe not, the others could be synthesized into a collection...?)

impl IoArgs {
    pub fn validate(&self) -> Result<()> {
        if self.in_place && self.input.to_str() == Some("-") {
            anyhow::bail!("Cannot use -i with stdin input");
        }
        Ok(())
    }

    pub fn read(&self) -> Result<String> {
        let mut input = String::new();
        if self.input.to_str() == Some("-") {
            std::io::stdin().read_to_string(&mut input)?;
            Ok(input)
        } else {
            Ok(std::fs::read_to_string(&self.input)?)
        }
    }

    pub fn is_collection(&self) -> bool {
        self.input.to_str() != Some("-") && self.input.is_dir()
    }

    /// If the input path is a directory, read a collection from it.
    pub fn as_collection(&self) -> Result<Collection<Outline>> {
        if self.is_collection() {
            Collection::load(&self.input)
        } else {
            bail!("Input is not a directory");
        }
    }

    pub fn write(&self, output: &str) -> Result<()> {
        match &self.output {
            None => {
                print!("{output}");
            }
            Some(x) if x.to_str() == Some("-") => {
                print!("{output}");
            }
            Some(ref outfile) => {
                std::fs::write(outfile, output)?;
            }
        }
        Ok(())
    }
}
