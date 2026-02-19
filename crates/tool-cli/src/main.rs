use std::fs;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use tool_core::{TextStats, analyze};
use tracing::debug;

/// A text analysis tool — analyze files for word counts, character stats, and more.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Files to analyze
    #[arg(required = true)]
    files: Vec<String>,

    /// Output format
    #[arg(long, default_value = "text", value_parser = ["json", "text"])]
    format: String,

    /// Enable verbose (debug) logging
    #[arg(long)]
    verbose: bool,
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    let filter = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    debug!("Parsed CLI args: {:?}", cli);

    let mut results: Vec<(&str, TextStats)> = Vec::new();

    for path in &cli.files {
        let content =
            fs::read_to_string(path).with_context(|| format!("Failed to read file: {path}"))?;
        let stats = analyze(&content);
        debug!(?stats, "Analyzed {}", path);
        results.push((path, stats));
    }

    match cli.format.as_str() {
        "json" => print_json(&results)?,
        "text" => print_text(&results),
        _ => unreachable!("clap validates format"),
    }

    Ok(ExitCode::SUCCESS)
}

fn print_json(results: &[(&str, TextStats)]) -> Result<()> {
    if results.len() == 1 {
        let json = serde_json::to_string_pretty(&results[0].1)?;
        println!("{json}");
    } else {
        let map: serde_json::Map<String, serde_json::Value> = results
            .iter()
            .map(|(path, stats)| ((*path).to_string(), serde_json::to_value(stats).unwrap()))
            .collect();
        let json = serde_json::to_string_pretty(&map)?;
        println!("{json}");
    }
    Ok(())
}

fn print_text(results: &[(&str, TextStats)]) {
    for (path, stats) in results {
        println!("--- {} ---", path);
        println!("  Lines:            {}", stats.lines);
        println!("  Words:            {}", stats.words);
        println!("  Characters:       {}", stats.chars);
        println!("  Bytes:            {}", stats.bytes);
        println!(
            "  Most common word: {}",
            stats.most_common_word.as_deref().unwrap_or("(none)")
        );
        println!("  Unique words:     {}", stats.unique_words);
        println!();
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)
        }
    }
}
