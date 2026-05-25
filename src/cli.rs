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
    /// Convert a NetCDF file to Parquet or YAML
    #[command(name = "convert")]
    Convert {
        #[command(subcommand)]
        format: ConvertFormat,
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
    /// NRT metadata (.nc -> .yaml)
    #[command(name = "nrt_head")]
    NrtHead {
        /// Source NetCDF file
        src: PathBuf,
        /// Output YAML file
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
    /// CORA metadata (.nc -> .yaml)
    #[command(name = "cora_head")]
    CoraHead {
        /// Source NetCDF file
        src: PathBuf,
        /// Output YAML file
        dest: PathBuf,
    },
}
