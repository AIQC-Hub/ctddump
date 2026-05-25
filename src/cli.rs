use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ctddump", version, about = "Convert CTD data from NetCDF to Parquet or YAML")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Convert a single NetCDF file to Parquet
    #[command(name = "convert")]
    Convert {
        #[command(subcommand)]
        format: ConvertFormat,
    },
    /// Batch-process all NetCDF files in a directory tree
    #[command(name = "batch")]
    Batch {
        #[command(subcommand)]
        subcommand: BatchSubcommand,
    },
    /// Extract metadata from a single NetCDF file to YAML
    #[command(name = "header")]
    Header {
        #[command(subcommand)]
        format: HeaderFormat,
    },
    /// Concatenate Parquet files
    #[command(name = "concat")]
    Concat {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

// ── convert ──────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ConvertFormat {
    /// NRT Arctic Sea (.nc -> .parquet)
    #[command(name = "nrt_ar")]
    NrtAr {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Baltic Sea (.nc -> .parquet)
    #[command(name = "nrt_bo")]
    NrtBo {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Mediterranean Sea (.nc -> .parquet)
    #[command(name = "nrt_mo")]
    NrtMo {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Global (.nc -> .parquet)
    #[command(name = "nrt_gl")]
    NrtGl {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
    /// CORA current format (.nc -> .parquet)
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
    /// CORA legacy format (.nc -> .parquet)
    #[command(name = "cora_legacy")]
    CoraLegacy {
        #[arg(short, long)] config: Option<PathBuf>,
        src: PathBuf,
        dest: PathBuf,
    },
}

// ── batch ─────────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum BatchSubcommand {
    /// Batch-convert .nc → .parquet for all files in a directory tree
    #[command(name = "convert")]
    Convert {
        #[command(subcommand)]
        format: BatchConvertFormat,
    },
    /// Batch-extract .nc → .yaml metadata for all files in a directory tree
    #[command(name = "header")]
    Header {
        #[command(subcommand)]
        format: BatchHeaderFormat,
    },
}

#[derive(Subcommand, Debug)]
pub enum BatchConvertFormat {
    /// NRT Arctic Sea: batch convert .nc → .parquet
    #[command(name = "nrt_ar")]
    NrtAr {
        #[arg(short, long)] config: Option<PathBuf>,
        /// Output directory (flat). Defaults to same directory as each input file.
        #[arg(short, long)] output: Option<PathBuf>,
        /// Number of threads (defaults to all available CPU cores)
        #[arg(short, long)] threads: Option<usize>,
        /// Source directory to search recursively for .nc files
        src_dir: PathBuf,
    },
    /// NRT Baltic Sea: batch convert .nc → .parquet
    #[command(name = "nrt_bo")]
    NrtBo {
        #[arg(short, long)] config: Option<PathBuf>,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// NRT Mediterranean Sea: batch convert .nc → .parquet
    #[command(name = "nrt_mo")]
    NrtMo {
        #[arg(short, long)] config: Option<PathBuf>,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// NRT Global: batch convert .nc → .parquet
    #[command(name = "nrt_gl")]
    NrtGl {
        #[arg(short, long)] config: Option<PathBuf>,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// CORA current format: batch convert .nc → .parquet
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] config: Option<PathBuf>,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// CORA legacy format: batch convert .nc → .parquet
    #[command(name = "cora_legacy")]
    CoraLegacy {
        #[arg(short, long)] config: Option<PathBuf>,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum BatchHeaderFormat {
    /// NRT: batch extract .nc → .yaml metadata
    #[command(name = "nrt")]
    Nrt {
        /// Output directory (flat). Defaults to same directory as each input file.
        #[arg(short, long)] output: Option<PathBuf>,
        /// Number of threads (defaults to all available CPU cores)
        #[arg(short, long)] threads: Option<usize>,
        /// Source directory to search recursively for .nc files
        src_dir: PathBuf,
    },
    /// CORA: batch extract .nc → .yaml metadata
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
}

// ── header ────────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum HeaderFormat {
    /// NRT metadata (.nc -> .yaml)
    #[command(name = "nrt")]
    Nrt {
        src: PathBuf,
        dest: PathBuf,
    },
    /// CORA metadata (.nc -> .yaml)
    #[command(name = "cora")]
    Cora {
        src: PathBuf,
        dest: PathBuf,
    },
}
