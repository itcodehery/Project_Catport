mod parser;
mod file_system;

use std::path::PathBuf;
use clap::Parser;
use parser::Cli;
use crate::parser::Commands;

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled.");
    }

    println!("{:?}", cli);

    match file_system::execute_view(PathBuf::from(match cli.command {
        Commands::View { file_name, .. } => { file_name },
        Commands::Share { .. } => { "".to_string() },
        Commands::Connect { .. } => { "".to_string()},
    })) {
        Ok(result) => {},
        Err(error) => {println!("{}", error); std::process::exit(1);}
    }
}
