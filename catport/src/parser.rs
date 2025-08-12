use clap::{Parser, Subcommand};

#[derive(Parser,Debug)]
#[command(name="catport")]
#[command(about="A modern cat replacement with live sharing")]
#[command(version="0.1.0")]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand,Debug)]
pub enum Commands {
    View {
        file_name: String,

        #[arg(long)]
        plain:bool,
    },
    Share {
        file_name: String,

        #[arg(long)]
        local_only: Option<bool>,
    },
    Connect {
        url : String
    },
}

