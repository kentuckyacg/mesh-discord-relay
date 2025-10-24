use clap::{Parser};

#[derive(Parser)]
#[clap(author = "KYACG", version = "0.0.1", about, long_about = None)]
#[derive(Debug)]
pub struct Args {
    /// Config file to use
    #[clap(short, long)]
    pub config_file: Option<String>,
    /// Enable debug output
    #[clap(long)]
    pub debug: bool,
    /// Enable verbose output
    #[clap(short, long)]
    pub verbose: bool,
}