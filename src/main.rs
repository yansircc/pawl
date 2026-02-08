mod cli;
mod cmd;
mod model;
mod util;
mod viewport;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cmd::dispatch(cli.command)
}
