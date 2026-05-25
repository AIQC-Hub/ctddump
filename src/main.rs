use clap::Parser;
use ctddump::cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = ctddump::run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
