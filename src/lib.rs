use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

pub mod batch;
pub mod cli;
pub mod concat;
pub mod convert;
pub mod header;

use cli::{Cli, Commands, ConvertFormat, BatchSubcommand, BatchConvertFormat, BatchHeaderFormat, HeaderFormat};
use concat::ConcatConfig;
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
        Commands::Concat { src_dir, output, pattern, no_renumber } => {
            let mut config = ConcatConfig::default();
            if let Some(p) = pattern { config.pattern = p; }
            if no_renumber { config.renumber = false; }
            concat::run_concat(&src_dir, &output, &config)?;
            Ok(Config {
                module: "concat".to_string(),
                target: output.to_string_lossy().into_owned(),
                args: vec![],
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
        ConvertFormat::NrtAr { config, nrt_args, src, dest } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_ar)?;
            nrt_args.apply_to(&mut nrt_config);
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_ar")
        }
        ConvertFormat::NrtBo { config, nrt_args, src, dest } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_bo)?;
            nrt_args.apply_to(&mut nrt_config);
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_bo")
        }
        ConvertFormat::NrtMo { config, nrt_args, src, dest } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_mo)?;
            nrt_args.apply_to(&mut nrt_config);
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_mo")
        }
        ConvertFormat::NrtGl { config, nrt_args, src, dest } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_gl)?;
            nrt_args.apply_to(&mut nrt_config);
            convert::nrt::run(&path_args(src, dest), nrt_config, "nrt_gl")
        }
        ConvertFormat::Cora { config, cora_args, src, dest } => {
            let mut cora_config = load_or_default(config, CoraConfig::cora)?;
            cora_args.apply_to(&mut cora_config);
            convert::cora::run(&path_args(src, dest), cora_config, "cora")
        }
        ConvertFormat::CoraLegacy { config, cora_args, src, dest } => {
            let mut cora_config = load_or_default(config, CoraConfig::cora_legacy)?;
            cora_args.apply_to(&mut cora_config);
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
        BatchConvertFormat::NrtAr { config, nrt_args, src_dir, output, threads } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_ar)?;
            nrt_args.apply_to(&mut nrt_config);
            let pattern = nrt_config.pattern.as_deref().unwrap_or("AR_PR_CT_*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_ar".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtBo { config, nrt_args, src_dir, output, threads } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_bo)?;
            nrt_args.apply_to(&mut nrt_config);
            let pattern = nrt_config.pattern.as_deref().unwrap_or("BO_PR_CT_*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_bo".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtMo { config, nrt_args, src_dir, output, threads } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_mo)?;
            nrt_args.apply_to(&mut nrt_config);
            let pattern = nrt_config.pattern.as_deref().unwrap_or("MO_PR_CT_*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_mo".to_string(), args: vec![] })
        }
        BatchConvertFormat::NrtGl { config, nrt_args, src_dir, output, threads } => {
            let mut nrt_config = load_or_default(config, NrtConfig::nrt_gl)?;
            nrt_args.apply_to(&mut nrt_config);
            let pattern = nrt_config.pattern.as_deref().unwrap_or("GL_PR_CT_*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::nrt::convert_file(src, dest, &nrt_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "nrt_gl".to_string(), args: vec![] })
        }
        BatchConvertFormat::Cora { config, cora_args, src_dir, output, threads } => {
            let mut cora_config = load_or_default(config, CoraConfig::cora)?;
            cora_args.apply_to(&mut cora_config);
            let pattern = cora_config.pattern.as_deref().unwrap_or("*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::cora::convert_file(src, dest, &cora_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "cora".to_string(), args: vec![] })
        }
        BatchConvertFormat::CoraLegacy { config, cora_args, src_dir, output, threads } => {
            let mut cora_config = load_or_default(config, CoraConfig::cora_legacy)?;
            cora_args.apply_to(&mut cora_config);
            let pattern = cora_config.pattern.as_deref().unwrap_or("*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "parquet", pattern, |src, dest| {
                convert::cora::convert_file(src, dest, &cora_config)
            })?;
            Ok(Config { module: "batch".to_string(), target: "cora_legacy".to_string(), args: vec![] })
        }
    }
}

fn dispatch_batch_header(format: BatchHeaderFormat) -> Result<Config, Box<dyn Error>> {
    match format {
        BatchHeaderFormat::Nrt { src_dir, output, threads, pattern } => {
            let effective_pattern = pattern.as_deref().unwrap_or("*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "yaml", effective_pattern, |src, dest| {
                header::nrt::extract_file(src, dest)
            })?;
            Ok(Config { module: "batch".to_string(), target: "header_nrt".to_string(), args: vec![] })
        }
        BatchHeaderFormat::Cora { src_dir, output, threads, pattern } => {
            let effective_pattern = pattern.as_deref().unwrap_or("*.nc");
            batch::run_batch(&src_dir, output.as_deref(), threads, "yaml", effective_pattern, |src, dest| {
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
