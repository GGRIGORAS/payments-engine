//! Thin CLI wrapper for the payments engine.
//!
//! ```text
//! payments_engine --input sample-data/transactions.csv --output accounts.csv
//! ```
//! If `--output` is omitted, results are written to STDOUT.

mod engine;
mod errors;
mod models;

use clap::Parser;
use csv::{ReaderBuilder, WriterBuilder};
use errors::Result;
use models::AccountRow;
use std::fs::File;
use std::io::{self, Write};

/// Stream a CSV of transactions and output per-client balances.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to input CSV (transactions).
    #[arg(short, long)]
    input: String,

    /// Path to output CSV (defaults to stdout).
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> Result<()> {
    // ----------------------------- CLI parsing
    let cli = Cli::parse();

    let infile = File::open(&cli.input)?;

    // ----------------------------- CSV ingest
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(infile);

    let mut engine = engine::Engine::new();

    for (idx, row) in rdr.deserialize().enumerate() {
        match row {
            Ok(tx) => engine.process(tx)?,
            Err(e) => eprintln!("⚠️  Skipping row {}: {e}", idx + 1),
        }
    }

    // ----------------------------- CSV output
    let sink: Box<dyn Write> = match cli.output {
        Some(path) => Box::new(File::create(path)?),
        None => Box::new(io::stdout()),
    };

    let mut wtr = WriterBuilder::new().has_headers(true).from_writer(sink);

    // deterministic order
    let mut clients: Vec<_> = engine.accounts.iter().collect();
    clients.sort_by_key(|(c, _)| *c);

    for (id, acc) in clients {
        wtr.serialize(AccountRow::from((id, acc)))?;
    }
    wtr.flush()?;
    Ok(())
}
