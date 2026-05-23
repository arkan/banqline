// Banqline — CLI + TUI for Enable Banking API
// Rust migration from Go codebase.

mod aggregator;
mod alerter;
mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod output;
mod session;
mod store;
mod tagger;
mod tui;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    commands::run(cli::Cli::parse()).await
}
