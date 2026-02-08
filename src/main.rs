mod cli;
mod cmd;
mod error;
mod model;
mod util;
mod viewport;

use clap::Parser;
use error::PawlError;

fn main() {
    let cli = cli::Cli::parse();
    match cmd::dispatch(cli.command) {
        Ok(()) => {}
        Err(e) => {
            if let Some(pe) = e.downcast_ref::<PawlError>() {
                eprintln!("{pe}");
                std::process::exit(pe.exit_code());
            } else {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
