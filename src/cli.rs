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
    /// Concatenate files from a directory tree into a single file
    #[command(name = "concat")]
    Concat {
        #[command(subcommand)]
        subcommand: ConcatSubcommand,
    },
}

// ── Concat subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ConcatSubcommand {
    /// Merge Parquet files into a single Parquet file
    #[command(name = "convert")]
    Convert {
        /// Source directory to search for Parquet files
        src_dir: PathBuf,
        /// Output Parquet file
        output: PathBuf,
        /// Glob pattern to match filenames (default: *.parquet)
        #[arg(long, value_name = "GLOB")]
        pattern: Option<String>,
        /// Do not re-assign profile_no and observation_no after merging
        #[arg(long = "no-renumber")]
        no_renumber: bool,
        /// Sort without `pres`: keep observations in their original per-profile
        /// order instead of reordering by pressure (ignored with --no-renumber)
        #[arg(long = "no-pres-sort")]
        no_pres_sort: bool,
        /// Keep rows with missing (null/NaN) `pres`. By default such rows are
        /// dropped before merging so observation_no stays contiguous
        #[arg(long = "keep-na-pres")]
        keep_na_pres: bool,
        /// Number of threads for parallel renumbering via temporary files in the
        /// output folder (defaults to all CPU cores; use 1 for the sequential,
        /// lowest-memory path; ignored with --no-renumber)
        #[arg(short, long)]
        threads: Option<usize>,
    },
    /// Merge header YAML files into a single YAML file
    #[command(name = "header")]
    Header {
        /// Source directory to search for YAML files
        src_dir: PathBuf,
        /// Output YAML file
        output: PathBuf,
        /// Glob pattern to match filenames (default: *.yaml)
        #[arg(long, value_name = "GLOB")]
        pattern: Option<String>,
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
    /// Glob pattern matched against filenames for batch file selection (e.g. "AR_PR_CT_*.nc")
    #[arg(long = "pattern", value_name = "GLOB")]
    pub pattern: Option<String>,
}

impl NrtArgs {
    /// Overwrite the fields of `config` for each flag that was explicitly set.
    /// Fields whose flag was not supplied are left unchanged.
    pub fn apply_to(&self, config: &mut NrtConfig) {
        if self.deph_source         { config.has_deph_source = true; }
        else if self.no_deph_source { config.has_deph_source = false; }
        if self.profile_coords         { config.has_profile_coords = true; }
        else if self.no_profile_coords { config.has_profile_coords = false; }
        if let Some(ref p) = self.pattern { config.pattern = Some(p.clone()); }
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
    /// Glob pattern matched against filenames for batch file selection (e.g. "CO_*.nc")
    #[arg(long = "pattern", value_name = "GLOB")]
    pub pattern: Option<String>,
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
        if let Some(ref p) = self.pattern { config.pattern = Some(p.clone()); }
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
        /// Glob pattern matched against filenames [default: "*.nc"]
        #[arg(long = "pattern", value_name = "GLOB")] pattern: Option<String>,
        /// Source directory to search recursively for .nc files
        src_dir: PathBuf,
    },
    /// CORA: batch extract .nc → .yaml metadata
    #[command(name = "cora")]
    Cora {
        #[arg(short, long)] output: Option<PathBuf>,
        #[arg(short, long)] threads: Option<usize>,
        /// Glob pattern matched against filenames [default: "*.nc"]
        #[arg(long = "pattern", value_name = "GLOB")] pattern: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::{CoraArgs, NrtArgs};
    use crate::convert::cora_config::{CoraConfig, QcType};
    use crate::convert::nrt_config::NrtConfig;

    fn nrt_args(
        deph_source: bool, no_deph_source: bool,
        profile_coords: bool, no_profile_coords: bool,
        pattern: Option<&str>,
    ) -> NrtArgs {
        NrtArgs { deph_source, no_deph_source, profile_coords, no_profile_coords,
                  pattern: pattern.map(str::to_string) }
    }

    fn cora_args(
        time_var: Option<&str>, qc_type: Option<QcType>,
        time_qc: bool, no_time_qc: bool,
        deph_source: bool, no_deph_source: bool,
        pattern: Option<&str>,
    ) -> CoraArgs {
        CoraArgs { time_var: time_var.map(str::to_string), qc_type,
                   time_qc, no_time_qc, deph_source, no_deph_source,
                   pattern: pattern.map(str::to_string) }
    }

    // ── NrtArgs::apply_to ────────────────────────────────────────────────────

    #[test]
    fn test_nrt_no_flags_leaves_config_unchanged() {
        let mut cfg = NrtConfig::nrt_bo(); // deph=true, profile=true
        nrt_args(false, false, false, false, None).apply_to(&mut cfg);
        assert!(cfg.has_deph_source);
        assert!(cfg.has_profile_coords);
        assert!(cfg.pattern.is_none());
    }

    #[test]
    fn test_nrt_deph_source_enables() {
        let mut cfg = NrtConfig::nrt_ar(); // deph=false
        nrt_args(true, false, false, false, None).apply_to(&mut cfg);
        assert!(cfg.has_deph_source);
    }

    #[test]
    fn test_nrt_no_deph_source_disables() {
        let mut cfg = NrtConfig::nrt_bo(); // deph=true
        nrt_args(false, true, false, false, None).apply_to(&mut cfg);
        assert!(!cfg.has_deph_source);
    }

    #[test]
    fn test_nrt_profile_coords_enables() {
        let mut cfg = NrtConfig::nrt_ar(); // profile=false
        nrt_args(false, false, true, false, None).apply_to(&mut cfg);
        assert!(cfg.has_profile_coords);
    }

    #[test]
    fn test_nrt_no_profile_coords_disables() {
        let mut cfg = NrtConfig::nrt_bo(); // profile=true
        nrt_args(false, false, false, true, None).apply_to(&mut cfg);
        assert!(!cfg.has_profile_coords);
    }

    #[test]
    fn test_nrt_pattern_sets_pattern() {
        let mut cfg = NrtConfig::nrt_ar();
        nrt_args(false, false, false, false, Some("MY_*.nc")).apply_to(&mut cfg);
        assert_eq!(cfg.pattern.as_deref(), Some("MY_*.nc"));
    }

    #[test]
    fn test_nrt_none_pattern_does_not_clear_existing() {
        let mut cfg = NrtConfig::nrt_ar();
        cfg.pattern = Some("OLD_*.nc".to_string());
        nrt_args(false, false, false, false, None).apply_to(&mut cfg);
        assert_eq!(cfg.pattern.as_deref(), Some("OLD_*.nc")); // unchanged
    }

    // ── CoraArgs::apply_to ───────────────────────────────────────────────────

    #[test]
    fn test_cora_no_flags_leaves_config_unchanged() {
        let mut cfg = CoraConfig::cora(); // time_var=TIME, qc=Int, time_qc=true, deph=true
        cora_args(None, None, false, false, false, false, None).apply_to(&mut cfg);
        assert_eq!(cfg.time_var, "TIME");
        assert_eq!(cfg.qc_type, QcType::Int);
        assert!(cfg.has_time_qc);
        assert!(cfg.has_deph_source);
    }

    #[test]
    fn test_cora_time_var_override() {
        let mut cfg = CoraConfig::cora(); // time_var=TIME
        cora_args(Some("JULD"), None, false, false, false, false, None).apply_to(&mut cfg);
        assert_eq!(cfg.time_var, "JULD");
    }

    #[test]
    fn test_cora_qc_type_override_to_char() {
        let mut cfg = CoraConfig::cora(); // qc_type=Int
        cora_args(None, Some(QcType::Char), false, false, false, false, None).apply_to(&mut cfg);
        assert_eq!(cfg.qc_type, QcType::Char);
    }

    #[test]
    fn test_cora_no_time_qc_disables() {
        let mut cfg = CoraConfig::cora(); // has_time_qc=true
        cora_args(None, None, false, true, false, false, None).apply_to(&mut cfg);
        assert!(!cfg.has_time_qc);
    }

    #[test]
    fn test_cora_time_qc_enables() {
        let mut cfg = CoraConfig::cora_legacy(); // has_time_qc=false
        cora_args(None, None, true, false, false, false, None).apply_to(&mut cfg);
        assert!(cfg.has_time_qc);
    }

    #[test]
    fn test_cora_deph_source_enables() {
        let mut cfg = CoraConfig::cora_legacy(); // has_deph_source=false
        cora_args(None, None, false, false, true, false, None).apply_to(&mut cfg);
        assert!(cfg.has_deph_source);
    }

    #[test]
    fn test_cora_no_deph_source_disables() {
        let mut cfg = CoraConfig::cora(); // has_deph_source=true
        cora_args(None, None, false, false, false, true, None).apply_to(&mut cfg);
        assert!(!cfg.has_deph_source);
    }

    #[test]
    fn test_cora_pattern_sets_pattern() {
        let mut cfg = CoraConfig::cora();
        cora_args(None, None, false, false, false, false, Some("CO_*.nc")).apply_to(&mut cfg);
        assert_eq!(cfg.pattern.as_deref(), Some("CO_*.nc"));
    }
}
