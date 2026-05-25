use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

pub mod cli;
pub mod netcdf;

use cli::{Cli, Commands, ConvertFormat};

// A struct to hold the configuration after parsing the arguments
#[derive(Debug, PartialEq)]
pub struct Config {
    pub module: String,
    pub target: String,
    pub args: Vec<String>,
}

/// Dispatches a parsed [`Cli`] to the appropriate converter.
/// Called directly from `main` after `Cli::parse()`.
pub fn run(cli: Cli) -> Result<Config, Box<dyn Error>> {
    match cli.command {
        Commands::Convert { format } => dispatch_convert(format),
        Commands::Concat { args } => {
            println!("Calling Concat module with arguments: {:?}", args);
            Ok(Config {
                module: "concat".to_string(),
                target: "".to_string(),
                args,
            })
        }
    }
}

/// Parses `args` with clap and dispatches to the appropriate converter.
/// Used by integration tests and as the library entry point.
pub fn handle_dispatch(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let cli = Cli::try_parse_from(
        std::iter::once("ctddump".to_string()).chain(args.iter().cloned()),
    )
    .map_err(|e| Box::new(e) as Box<dyn Error>)?;

    run(cli)
}

fn dispatch_convert(format: ConvertFormat) -> Result<Config, Box<dyn Error>> {
    match format {
        ConvertFormat::NrtAr { src, dest } => netcdf::nrt_ar::run(&path_args(src, dest)),
        ConvertFormat::NrtBo { src, dest } => netcdf::nrt_bo::run(&path_args(src, dest)),
        ConvertFormat::NrtMo { src, dest } => netcdf::nrt_mo::run(&path_args(src, dest)),
        ConvertFormat::NrtGl { src, dest } => netcdf::nrt_gl::run(&path_args(src, dest)),
        ConvertFormat::NrtHead { src, dest } => netcdf::nrt_head::run(&path_args(src, dest)),
        ConvertFormat::Cora { src, dest } => netcdf::cora::run(&path_args(src, dest)),
        ConvertFormat::CoraLegacy { src, dest } => netcdf::cora_legacy::run(&path_args(src, dest)),
        ConvertFormat::CoraHead { src, dest } => netcdf::cora_head::run(&path_args(src, dest)),
    }
}

fn path_args(src: PathBuf, dest: PathBuf) -> Vec<String> {
    vec![
        src.to_string_lossy().into_owned(),
        dest.to_string_lossy().into_owned(),
    ]
}
