use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

pub mod cli;
pub mod convert;

use cli::{Cli, Commands, ConvertFormat};
use convert::cora_config::CoraConfig;
use convert::nrt_config::NrtConfig;

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
        ConvertFormat::NrtAr { config, src, dest } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_ar)?;
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_ar")
        }
        ConvertFormat::NrtBo { config, src, dest } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_bo)?;
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_bo")
        }
        ConvertFormat::NrtMo { config, src, dest } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_mo)?;
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_mo")
        }
        ConvertFormat::NrtGl { config, src, dest } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_gl)?;
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_gl")
        }
        ConvertFormat::NrtHead { src, dest } => {
            convert::nrt_head::run(&path_args(src, dest))
        }
        ConvertFormat::Cora { config, src, dest } => {
            let cora_config = load_or_default(config, CoraConfig::cora)?;
            convert::cora::run(&path_args(src, dest), cora_config, "cora")
        }
        ConvertFormat::CoraLegacy { config, src, dest } => {
            let cora_config = load_or_default(config, CoraConfig::cora_legacy)?;
            convert::cora::run(&path_args(src, dest), cora_config, "cora_legacy")
        }
        ConvertFormat::CoraHead { src, dest } => {
            convert::cora_head::run(&path_args(src, dest))
        }
    }
}

/// Returns a config loaded from `path` if provided, or the embedded default otherwise.
fn load_or_default<T, F>(path: Option<PathBuf>, default: F) -> Result<T, Box<dyn Error>>
where
    F: Fn() -> T,
    T: for<'de> serde::Deserialize<'de>,
{
    match path {
        Some(p) => {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| format!("Cannot read config file {}: {}", p.display(), e))?;
            toml::from_str(&content)
                .map_err(|e| format!("Invalid config file {}: {}", p.display(), e).into())
        }
        None => Ok(default()),
    }
}

fn path_args(src: PathBuf, dest: PathBuf) -> Vec<String> {
    vec![
        src.to_string_lossy().into_owned(),
        dest.to_string_lossy().into_owned(),
    ]
}
