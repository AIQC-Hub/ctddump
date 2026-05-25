use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

pub mod batch;
pub mod cli;
pub mod convert;
pub mod header;

use cli::{Cli, Commands, ConvertFormat, BatchSubcommand, BatchConvertFormat, BatchHeaderFormat, HeaderFormat};
use convert::cora_config::CoraConfig;
use convert::nrt_config::NrtConfig;

// A struct to hold the configuration after parsing the arguments
#[derive(Debug, PartialEq)]
pub struct Config {
    pub module: String,
    pub target: String,
    pub args: Vec<String>,
}

/// Dispatches a parsed [`Cli`] to the appropriate module.
/// Called directly from `main` after `Cli::parse()`.
pub fn run(cli: Cli) -> Result<Config, Box<dyn Error>> {
    match cli.command {
        Commands::Convert { format } => dispatch_convert(format),
        Commands::Batch { subcommand } => dispatch_batch(subcommand),
        Commands::Header { format } => dispatch_header(format),
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

/// Parses `args` with clap and dispatches to the appropriate module.
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
        ConvertFormat::Cora { config, src, dest } => {
            let cora_config = load_or_default(config, CoraConfig::cora)?;
            convert::cora::run(&path_args(src, dest), cora_config, "cora")
        }
        ConvertFormat::CoraLegacy { config, src, dest } => {
            let cora_config = load_or_default(config, CoraConfig::cora_legacy)?;
            convert::cora::run(&path_args(src, dest), cora_config, "cora_legacy")
        }
    }
}

fn dispatch_batch(subcommand: BatchSubcommand) -> Result<Config, Box<dyn Error>> {
    match subcommand {
        BatchSubcommand::Convert { format } => dispatch_batch_convert(format),
        BatchSubcommand::Header { format } => dispatch_batch_header(format),
    }
}

fn dispatch_batch_convert(format: BatchConvertFormat) -> Result<Config, Box<dyn Error>> {
    match format {
        BatchConvertFormat::NrtAr { config, src_dir, output, threads } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_ar)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_ar".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtBo { config, src_dir, output, threads } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_bo)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_bo".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtMo { config, src_dir, output, threads } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_mo)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_mo".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtGl { config, src_dir, output, threads } => {
            let nrt_config = load_or_default(config, NrtConfig::nrt_gl)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_gl".to_string(), args: vec![] })
        }
        BatchConvertFormat::Cora { config, src_dir, output, threads } => {
            let cora_config = load_or_default(config, CoraConfig::cora)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::cora::convert_file(src, dest, &cora_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "cora".to_string(), args: vec![] })
        }
        BatchConvertFormat::CoraLegacy { config, src_dir, output, threads } => {
            let cora_config = load_or_default(config, CoraConfig::cora_legacy)?;
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", |src, dest| {
                convert::cora::convert_file(src, dest, &cora_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "cora_legacy".to_string(), args: vec![] })
        }
    }
}

fn dispatch_batch_header(format: BatchHeaderFormat) -> Result<Config, Box<dyn Error>> {
    match format {
        BatchHeaderFormat::Nrt { src_dir, output, threads } => {
            batch::run_batch(&src_dir, output.as_deref(), threads, "yaml", |src, dest| {
                header::nrt::extract_file(src, dest)
            })?;
            Ok(Config { module: "batch".to_string(), target: "header_nrt".to_string(), args: vec![] })
        }
        BatchHeaderFormat::Cora { src_dir, output, threads } => {
            batch::run_batch(&src_dir, output.as_deref(), threads, "yaml", |src, dest| {
                header::cora::extract_file(src, dest)
            })?;
            Ok(Config { module: "batch".to_string(), target: "header_cora".to_string(), args: vec![] })
        }
    }
}

fn dispatch_header(format: HeaderFormat) -> Result<Config, Box<dyn Error>> {
    match format {
        HeaderFormat::Nrt { src, dest } => header::nrt::run(&path_args(src, dest)),
        HeaderFormat::Cora { src, dest } => header::cora::run(&path_args(src, dest)),
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
