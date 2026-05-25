use std::error::Error;
use std::path::{Path, PathBuf};

use glob::Pattern;
use polars::prelude::*;
use walkdir::WalkDir;

/// Configuration for the `concat` command.
#[derive(Debug, Clone)]
pub struct ConcatConfig {
    /// Glob pattern matched against filenames only (default: `*.parquet`).
    pub pattern: String,
    /// Re-assign `profile_no` and `observation_no` after merging (default: `true`).
    pub renumber: bool,
}

impl Default for ConcatConfig {
    fn default() -> Self {
        ConcatConfig {
            pattern: "*.parquet".to_string(),
            renumber: true,
        }
    }
}

/// Recursively collect all files under `src_dir` whose filename matches `pattern`.
fn collect_parquet_files(src_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let glob_pat = Pattern::new(pattern)
        .map_err(|e| format!("Invalid pattern '{}': {}", pattern, e))?;

    let mut files: Vec<PathBuf> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |name| glob_pat.matches(name))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if files.is_empty() {
        return Err(format!(
            "No files matching '{}' found under {}",
            pattern,
            src_dir.display()
        )
        .into());
    }

    // Deterministic order across platforms
    files.sort();
    Ok(files)
}

/// Sort the combined DataFrame and re-assign `profile_no` and `observation_no`.
///
/// Sort order: `platform_code`, `profile_timestamp`, `longitude`, `latitude`, `pres`.
///
/// `profile_no` — dense rank of the composite key
/// `(platform_code | profile_timestamp | longitude | latitude)` within each
/// `platform_code`.  Profiles sharing the same key (same platform, time, and
/// position) receive the same `profile_no`.
///
/// `observation_no` — 1-based sequential counter within each
/// `(platform_code, profile_no)` group, in the sorted row order.
fn renumber(lf: LazyFrame) -> LazyFrame {
    lf.sort(
        ["platform_code", "profile_timestamp", "longitude", "latitude", "pres"],
        SortMultipleOptions::default(),
    )
    .with_column(
        concat_str(
            [
                col("platform_code"),
                col("profile_timestamp").cast(DataType::String),
                col("longitude").cast(DataType::String),
                col("latitude").cast(DataType::String),
            ],
            "|",
            false,
        )
        .alias("_profile_key"),
    )
    .with_column(
        col("_profile_key")
            .rank(
                RankOptions {
                    method: RankMethod::Dense,
                    descending: false,
                },
                None,
            )
            .over(["platform_code"])
            .cast(DataType::UInt32)
            .alias("profile_no"),
    )
    .with_column(
        // platform_code is always non-null, so cum_count gives 1-based sequential
        // row numbers within each (platform_code, profile_no) group.
        col("platform_code")
            .cum_count(false)
            .over(["platform_code", "profile_no"])
            .cast(DataType::UInt32)
            .alias("observation_no"),
    )
    .drop(["_profile_key"])
}

/// Concatenate all Parquet files matching `config.pattern` under `src_dir` into
/// a single Parquet file at `output_file`.
///
/// When `config.renumber` is `true` (the default), the merged DataFrame is
/// sorted and `profile_no` / `observation_no` are reassigned so that profile
/// numbers are globally unique and sequential within each platform.
///
/// Returns the number of input files merged.
pub fn run_concat(
    src_dir: &Path,
    output_file: &Path,
    config: &ConcatConfig,
) -> Result<usize, Box<dyn Error>> {
    let files = collect_parquet_files(src_dir, &config.pattern)?;
    let n = files.len();

    let frames: Vec<LazyFrame> = files
        .iter()
        .map(|f| {
            let file = std::fs::File::open(f)
                .map_err(|e| format!("Cannot open {}: {}", f.display(), e))?;
            Ok(ParquetReader::new(file).finish()?.lazy())
        })
        .collect::<Result<_, Box<dyn Error>>>()?;

    let combined = concat(frames, UnionArgs::default())?;

    let combined = if config.renumber {
        renumber(combined)
    } else {
        combined
    };

    let mut result = combined.collect()?;

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create output directory: {}", e))?;
        }
    }

    let mut out_file = std::fs::File::create(output_file)
        .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;
    ParquetWriter::new(&mut out_file).finish(&mut result)?;

    Ok(n)
}
