mod commands;
mod parser;

use crate::commands::share::start_sharing;
use crate::parser::Commands;
use clap::Parser;
use commands::view;
use parser::Cli;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled.");
    }

    match cli.command {
        Commands::Connect { url: _ } => {}
        Commands::Share {
            file_name,
            local_only: _,
        } => {
            let _ = start_sharing(PathBuf::from(file_name)).await;
        }
        Commands::View { file_name, plain } => {
            match view::execute_view(PathBuf::from(file_name), plain) {
                Ok(_) => {}
                Err(err) => {
                    println!("ERROR: {}", err)
                }
            };
        }
    }
}
