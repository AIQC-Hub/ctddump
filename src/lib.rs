use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

pub mod batch;
pub mod cli;
pub mod concat;
pub mod convert;
pub mod dedup;
pub mod dropna;
pub mod dropqc;
pub mod dupkey;
pub mod filter;
pub mod header;
pub mod markdup;
pub mod report;

use cli::{Cli, Commands, ConvertFormat, BatchSubcommand, BatchConvertFormat, BatchHeaderFormat, HeaderFormat, ConcatSubcommand, ReportSubcommand, FilterMode};
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
        Commands::Concat { subcommand } => dispatch_concat(subcommand),
        Commands::Report { subcommand } => dispatch_report(subcommand),
        Commands::Filter { mode, min_lon, max_lon, min_lat, max_lat, src, dest } => {
            dispatch_filter(mode, min_lon, max_lon, min_lat, max_lat, src, dest)
        }
        Commands::Dropna { src, dest } => {
            dropna::run(&src, &dest)?;
            Ok(Config { module: "dropna".to_string(), target: "parquet".to_string(), args: vec![] })
        }
        Commands::Dropqc { src, dest } => {
            dropqc::run(&src, &dest)?;
            Ok(Config { module: "dropqc".to_string(), target: "parquet".to_string(), args: vec![] })
        }
        Commands::Markdup { time_format, decimals, round_mode, src, dest, dups } => {
            let opts = dupkey::KeyOpts { time_format, decimals, round_mode };
            markdup::run(&opts, &src, &dest, &dups)?;
            Ok(Config { module: "markdup".to_string(), target: "parquet".to_string(), args: vec![] })
        }
        Commands::Dedup { time_format, decimals, round_mode, src, dest } => {
            let opts = dupkey::KeyOpts { time_format, decimals, round_mode };
            dedup::run(&opts, &src, &dest)?;
            Ok(Config { module: "dedup".to_string(), target: "parquet".to_string(), args: vec![] })
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

fn dispatch_concat(subcommand: ConcatSubcommand) -> Result<Config, Box<dyn Error>> {
    match subcommand {
        ConcatSubcommand::Convert { src_dir, output, pattern, no_renumber, no_pres_sort, keep_na_pres, threads } => {
            let mut config = ConcatConfig::default();
            if let Some(p) = pattern { config.pattern = p; }
            if no_renumber { config.renumber = false; }
            if no_pres_sort { config.sort_by_pres = false; }
            if keep_na_pres { config.drop_na_pres = false; }
            config.threads = threads;
            concat::run_concat_parquet(&src_dir, &output, &config)?;
            Ok(Config { module: "concat".to_string(), target: "convert".to_string(), args: vec![] })
        }
        ConcatSubcommand::Header { src_dir, output, pattern } => {
            let effective_pattern = pattern.as_deref().unwrap_or("*.yaml");
            concat::run_concat_header(&src_dir, &output, effective_pattern)?;
            Ok(Config { module: "concat".to_string(), target: "header".to_string(), args: vec![] })
        }
    }
}

fn dispatch_report(subcommand: ReportSubcommand) -> Result<Config, Box<dyn Error>> {
    match subcommand {
        ReportSubcommand::Parquet { level, format, src, dest } => {
            report::parquet::run(level, format, &src, dest.as_deref())?;
            Ok(Config { module: "report".to_string(), target: "parquet".to_string(), args: vec![] })
        }
        ReportSubcommand::Yaml { format, src, dest } => {
            report::yaml::run(format, &src, dest.as_deref())?;
            Ok(Config { module: "report".to_string(), target: "yaml".to_string(), args: vec![] })
        }
        ReportSubcommand::Summary { stem, report_dir, out_dir, format, title, note, output } => {
            report::summary::run(
                &stem,
                report::summary::Opts {
                    report_dir: &report_dir,
                    out_dir: &out_dir,
                    format,
                    title: title.as_deref(),
                    notes: &note,
                    output: output.as_deref(),
                },
            )?;
            Ok(Config { module: "report".to_string(), target: "summary".to_string(), args: vec![] })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_filter(
    mode: FilterMode,
    min_lon: f64,
    max_lon: f64,
    min_lat: f64,
    max_lat: f64,
    src: PathBuf,
    dest: PathBuf,
) -> Result<Config, Box<dyn Error>> {
    filter::run(mode, min_lon, max_lon, min_lat, max_lat, &src, &dest)?;
    Ok(Config { module: "filter".to_string(), target: "area".to_string(), args: vec![] })
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
