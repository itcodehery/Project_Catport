mod commands;
mod parser;

use crate::parser::Commands;
use clap::Parser;
use parser::Cli;
use std::path::PathBuf;
use commands::view;
use crate::commands::share::start_sharing;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled.");
    }

    match cli.command {
        Commands::Connect { url } => {}
        Commands::Share {
            file_name,
            local_only,
        } => {
            start_sharing(PathBuf::from(file_name)).await;
        }
        Commands::View { file_name, plain } => {
            match view::execute_view(PathBuf::from(file_name), plain) {
                Ok(res) => {
                    println!("The view command has successfully been implemented!")
                }
                Err(err) => {
                    println!("ERROR: {}", err)
                }
            };
        }
    }
}
