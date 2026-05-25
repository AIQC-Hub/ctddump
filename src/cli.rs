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
    /// Convert a NetCDF file to Parquet
    #[command(name = "convert")]
    Convert {
        #[command(subcommand)]
        format: ConvertFormat,
    },
    /// Extract metadata from a NetCDF file to YAML
    #[command(name = "header")]
    Header {
        #[command(subcommand)]
        format: HeaderFormat,
    },
    /// Batch-convert all NetCDF files in a directory tree to Parquet
    #[command(name = "batch")]
    Batch {
        #[command(subcommand)]
        format: BatchFormat,
    },
    /// Concatenate Parquet files
    #[command(name = "concat")]
    Concat {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConvertFormat {
    /// NRT Arctic Sea (.nc -> .parquet)
    #[command(name = "nrt_ar")]
    NrtAr {
        /// Optional TOML config file to override default NRT settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
    /// NRT Baltic Sea (.nc -> .parquet)
    #[command(name = "nrt_bo")]
    NrtBo {
        /// Optional TOML config file to override default NRT settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
    /// NRT Mediterranean Sea (.nc -> .parquet)
    #[command(name = "nrt_mo")]
    NrtMo {
        /// Optional TOML config file to override default NRT settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
    /// NRT Global (.nc -> .parquet)
    #[command(name = "nrt_gl")]
    NrtGl {
        /// Optional TOML config file to override default NRT settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
    /// CORA current format (.nc -> .parquet)
    #[command(name = "cora")]
    Cora {
        /// Optional TOML config file to override default CORA settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
    /// CORA legacy format (.nc -> .parquet)
    #[command(name = "cora_legacy")]
    CoraLegacy {
        /// Optional TOML config file to override default CORA legacy settings
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Source NetCDF file
        src: PathBuf,
        /// Output Parquet file
        dest: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum BatchFormat {
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
pub enum HeaderFormat {
    /// NRT metadata (.nc -> .yaml)
    #[command(name = "nrt")]
    Nrt {
        /// Source NetCDF file
        src: PathBuf,
        /// Output YAML file
        dest: PathBuf,
    },
    /// CORA metadata (.nc -> .yaml)
    #[command(name = "cora")]
    Cora {
        /// Source NetCDF file
        src: PathBuf,
        /// Output YAML file
        dest: PathBuf,
    },
}
