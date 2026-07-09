use clap::Parser;
use ctddump::cli::Cli;

fn main() {
    // Worker threads (rayon's batch pool *and* Polars' internal pool) are spawned
    // with a 2 MiB stack by default — they do not inherit the main thread's 8 MiB.
    // Large files overflow that inside Polars' parquet writer, so raise the default
    // stack for every thread std spawns. Must be set before any pool initializes,
    // and only when the user hasn't chosen their own value.
    if std::env::var_os("RUST_MIN_STACK").is_none() {
        std::env::set_var("RUST_MIN_STACK", (16 * 1024 * 1024).to_string());
    }

    let cli = Cli::parse();
    if let Err(e) = ctddump::run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
