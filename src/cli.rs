use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::convert::cora_config::{CoraConfig, QcType};
use crate::convert::nrt_config::NrtConfig;

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

// ── NRT config overrides ──────────────────────────────────────────────────────

/// Per-field overrides for NRT configuration.
/// Applied on top of `--config` (or the built-in default) in priority order:
/// built-in default < `--config` file < individual flags.
#[derive(clap::Args, Debug, Clone)]
pub struct NrtArgs {
    /// Source contains a DEPH variable
    #[arg(long = "deph-source", overrides_with = "no_deph_source")]
    pub deph_source: bool,
    /// Source has no DEPH variable
    #[arg(long = "no-deph-source", overrides_with = "deph_source")]
    pub no_deph_source: bool,
    /// Derive profile_longitude/profile_latitude from PRECISE_* or DEPLOY_* coords
    #[arg(long = "profile-coords", overrides_with = "no_profile_coords")]
    pub profile_coords: bool,
    /// Do not derive profile coordinate columns
    #[arg(long = "no-profile-coords", overrides_with = "profile_coords")]
    pub no_profile_coords: bool,
}

impl NrtArgs {
    /// Overwrite the fields of `config` for each flag that was explicitly set.
    /// Fields whose flag was not supplied are left unchanged.
    pub fn apply_to(&self, config: &mut NrtConfig) {
        if self.deph_source         { config.has_deph_source = true; }
        else if self.no_deph_source { config.has_deph_source = false; }
        if self.profile_coords         { config.has_profile_coords = true; }
        else if self.no_profile_coords { config.has_profile_coords = false; }
    }
}

// ── CORA config overrides ─────────────────────────────────────────────────────

/// Per-field overrides for CORA configuration.
/// Applied on top of `--config` (or the built-in default) in priority order:
/// built-in default < `--config` file < individual flags.
#[derive(clap::Args, Debug, Clone)]
pub struct CoraArgs {
    /// Time variable name in the source file (e.g. TIME, JULD)
    #[arg(long = "time-var", value_name = "VAR")]
    pub time_var: Option<String>,
    /// QC flag storage type in the source file
    #[arg(long = "qc-type")]
    pub qc_type: Option<QcType>,
    /// Source has TIME_QC/POSITION_QC variables
    #[arg(long = "time-qc", overrides_with = "no_time_qc")]
    pub time_qc: bool,
    /// Source has no TIME_QC/POSITION_QC variables
    #[arg(long = "no-time-qc", overrides_with = "time_qc")]
    pub no_time_qc: bool,
    /// Source contains a DEPH variable
    #[arg(long = "deph-source", overrides_with = "no_deph_source")]
    pub deph_source: bool,
    /// Source has no DEPH variable
    #[arg(long = "no-deph-source", overrides_with = "deph_source")]
    pub no_deph_source: bool,
}

impl CoraArgs {
    /// Overwrite the fields of `config` for each flag that was explicitly set.
    /// Fields whose flag was not supplied are left unchanged.
    pub fn apply_to(&self, config: &mut CoraConfig) {
        if let Some(ref tv) = self.time_var  { config.time_var  = tv.clone(); }
        if let Some(ref qt) = self.qc_type   { config.qc_type   = qt.clone(); }
        if self.time_qc         { config.has_time_qc = true; }
        else if self.no_time_qc { config.has_time_qc = false; }
        if self.deph_source         { config.has_deph_source = true; }
        else if self.no_deph_source { config.has_deph_source = false; }
    }
}

// ── convert ──────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ConvertFormat {
    /// NRT Arctic Sea (.nc -> .parquet)
    #[command(name = "nrt_ar")]
    NrtAr {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Baltic Sea (.nc -> .parquet)
    #[command(name = "nrt_bo")]
    NrtBo {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Mediterranean Sea (.nc -> .parquet)
    #[command(name = "nrt_mo")]
    NrtMo {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        src: PathBuf,
        dest: PathBuf,
    },
    /// NRT Global (.nc -> .parquet)
    #[command(name = "nrt_gl")]
    NrtGl {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        src: PathBuf,
        dest: PathBuf,
    },
    /// CORA current format (.nc -> .parquet)
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] cora_args: CoraArgs,
        src: PathBuf,
        dest: PathBuf,
    },
    /// CORA legacy format (.nc -> .parquet)
    #[command(name = "cora_legacy")]
    CoraLegacy {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] cora_args: CoraArgs,
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
        #[command(flatten)] nrt_args: NrtArgs,
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
        #[command(flatten)] nrt_args: NrtArgs,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// NRT Mediterranean Sea: batch convert .nc → .parquet
    #[command(name = "nrt_mo")]
    NrtMo {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// NRT Global: batch convert .nc → .parquet
    #[command(name = "nrt_gl")]
    NrtGl {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] nrt_args: NrtArgs,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// CORA current format: batch convert .nc → .parquet
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] cora_args: CoraArgs,
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        src_dir: PathBuf,
    },
    /// CORA legacy format: batch convert .nc → .parquet
    #[command(name = "cora_legacy")]
    CoraLegacy {
        #[arg(short, long)] config: Option<PathBuf>,
        #[command(flatten)] cora_args: CoraArgs,
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
