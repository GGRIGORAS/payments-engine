//! CLI wrapper that supports both:
//!   cargo run -- transactions.csv > accounts.csv
//!   cargo run -- --input transactions.csv --output accounts.csv

use anyhow::Result;
use clap::{Arg, Command};
use csv::{ReaderBuilder, WriterBuilder};
use payments_engine::Engine;
use std::{
    env,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    // ---------------------------------------------------------------- logging
    let subscriber = FmtSubscriber::builder()
        .with_target(false)
        .with_writer(io::stderr) // logs → STDERR
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // ---------------------------------------------------------------- flags
    let matches = Command::new("payments-engine")
        .arg(
            Arg::new("input")
                .long("input")
                .value_name("FILE")
                .help("Input transactions CSV"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .value_name("FILE")
                .help("Output accounts CSV (defaults to stdout)"),
        )
        .disable_help_subcommand(true)
        .allow_external_subcommands(true)
        .get_matches();

    // ---------------------------------------------------- positional fallback
    let pos: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();

    let in_path = matches
        .get_one::<String>("input")
        .map(PathBuf::from)
        // only clippy::get_first cares about index 0
        .or_else(|| pos.first().cloned());

    let out_path = matches
        .get_one::<String>("output")
        .map(PathBuf::from)
        .or_else(|| pos.get(1).cloned()); // index 1 is fine

    let infile = match in_path {
        Some(p) => File::open(&p)?,
        None => {
            eprintln!("Usage: cargo run -- transactions.csv > accounts.csv");
            std::process::exit(1);
        }
    };

    // ---------------------------------------------------------------- ingest
    let mut rdr = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(infile);

    let mut engine = Engine::new();
    for (idx, row) in rdr.deserialize().enumerate() {
        match row {
            Ok(tx) => engine.process(tx)?,
            Err(e) => error!(row = idx + 1, %e, "csv-deserialize"),
        }
    }
    info!("Finished ingest: {} accounts", engine.accounts.len());

    // ---------------------------------------------------------------- emit
    let sink: Box<dyn Write> = match out_path {
        Some(p) => Box::new(File::create(p)?),
        None => Box::new(io::stdout()),
    };

    let mut wtr = WriterBuilder::new().has_headers(true).from_writer(sink);

    // header row (no needless borrow)
    wtr.write_record(["client", "available", "held", "total", "locked"])?;

    let mut clients: Vec<_> = engine.accounts.iter().collect();
    clients.sort_by_key(|(id, _)| *id);

    for (id, acc) in clients {
        wtr.write_record(&[
            id.to_string(),
            format!("{:.4}", acc.available),
            format!("{:.4}", acc.held),
            format!("{:.4}", acc.total()),
            acc.locked.to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}
