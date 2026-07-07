use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use glob::Pattern;
use polars::prelude::*;
use walkdir::WalkDir;

use crate::convert::common;

/// Configuration for the `concat` command.
#[derive(Debug, Clone)]
pub struct ConcatConfig {
    /// Glob pattern matched against filenames only (default: `*.parquet`).
    pub pattern: String,
    /// Re-assign `profile_no` and `observation_no` after merging (default: `true`).
    pub renumber: bool,
    /// Include `pres` as the final sort key when renumbering (default: `true`).
    ///
    /// When `false`, rows are sorted by `platform_code, profile_timestamp,
    /// longitude, latitude` only and the sort is stable, so observations keep
    /// their original per-profile order from the source files instead of being
    /// reordered by pressure. Ignored when `renumber` is `false`.
    pub sort_by_pres: bool,
}

impl Default for ConcatConfig {
    fn default() -> Self {
        ConcatConfig {
            pattern: "*.parquet".to_string(),
            renumber: true,
            sort_by_pres: true,
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
/// Sort order: `platform_code`, `profile_timestamp`, `longitude`, `latitude`, and
/// — when `sort_by_pres` is `true` — `pres`. When `sort_by_pres` is `false` the
/// `pres` key is dropped and the sort is made stable (`maintain_order`), so
/// observations keep their original per-profile order from the source rather than
/// being reordered by pressure.
///
/// `profile_no` — dense rank of the composite key
/// `(platform_code | profile_timestamp | longitude | latitude)` within each
/// `platform_code`.  Profiles sharing the same key (same platform, time, and
/// position) receive the same `profile_no`.
///
/// `observation_no` — 1-based sequential counter within each
/// `(platform_code, profile_no)` group, in the sorted row order.
fn renumber(lf: LazyFrame, sort_by_pres: bool) -> LazyFrame {
    let mut sort_keys: Vec<&str> =
        vec!["platform_code", "profile_timestamp", "longitude", "latitude"];
    if sort_by_pres {
        sort_keys.push("pres");
    }
    // Without `pres` as a tie-breaker, keep equal-key rows in their incoming
    // (source) order so `observation_no` is deterministic and follows the original
    // observation order.
    let sort_opts = SortMultipleOptions::default().with_maintain_order(!sort_by_pres);

    lf.sort(sort_keys, sort_opts)
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

/// Per-platform index built by the cheap first pass over the inputs.
struct PlatformIndex {
    /// Total observation rows per `platform_code`, summed across all input files.
    counts: HashMap<String, u64>,
    /// `(file, min_platform, max_platform)` for every non-empty input file, used to
    /// skip files that cannot contain a given platform range in the second pass.
    spans: Vec<(PathBuf, String, String)>,
}

/// First pass: scan only the `platform_code` column of every input file (projection
/// keeps this cheap) and record per-platform row counts plus each file's min/max
/// platform. Empty files contribute nothing.
fn scan_platform_index(files: &[PathBuf]) -> Result<PlatformIndex, Box<dyn Error>> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut spans: Vec<(PathBuf, String, String)> = Vec::new();

    for path in files {
        let df = LazyFrame::scan_parquet(path, ScanArgsParquet::default())
            .map_err(|e| format!("Cannot scan {}: {}", path.display(), e))?
            .select([col("platform_code")])
            .group_by([col("platform_code")])
            .agg([len().alias("n")])
            .collect()
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

        let pc = df.column("platform_code")?.str()?;
        let n = df.column("n")?.cast(&DataType::UInt64)?;
        let n = n.u64()?;

        let mut file_min: Option<String> = None;
        let mut file_max: Option<String> = None;
        for (name, cnt) in pc.into_iter().zip(n.into_iter()) {
            // platform_code is always non-null; len() is always non-null.
            let (name, cnt) = (name.unwrap(), cnt.unwrap());
            *counts.entry(name.to_string()).or_insert(0) += cnt;
            if file_min.as_deref().map_or(true, |m| name < m) {
                file_min = Some(name.to_string());
            }
            if file_max.as_deref().map_or(true, |m| name > m) {
                file_max = Some(name.to_string());
            }
        }
        if let (Some(lo), Some(hi)) = (file_min, file_max) {
            spans.push((path.clone(), lo, hi));
        }
    }

    Ok(PlatformIndex { counts, spans })
}

/// Partition the distinct platforms into contiguous `[lo, hi]` ranges (ascending
/// `platform_code` order) so each range holds at most `budget` observation rows.
/// A single platform larger than `budget` forms its own range (it is the smallest
/// unit that `renumber` can process independently).
fn partition_platform_ranges(
    counts: &HashMap<String, u64>,
    budget: u64,
) -> Vec<(String, String)> {
    let mut platforms: Vec<&String> = counts.keys().collect();
    platforms.sort();

    let mut ranges = Vec::new();
    let mut i = 0;
    while i < platforms.len() {
        let lo = platforms[i].clone();
        let mut sum = counts[platforms[i]];
        let mut j = i;
        while j + 1 < platforms.len() && sum + counts[platforms[j + 1]] <= budget {
            j += 1;
            sum += counts[platforms[j]];
        }
        ranges.push((lo, platforms[j].clone()));
        i = j + 1;
    }
    ranges
}

/// Concatenate all Parquet files matching `config.pattern` under `src_dir` into
/// a single Parquet file at `output_file`.
///
/// When `config.renumber` is `true` (the default), the merged data is sorted and
/// `profile_no` / `observation_no` are reassigned so that profile numbers are
/// globally unique and sequential within each platform.
///
/// Memory is bounded: rather than loading every file at once, the inputs are
/// processed one contiguous `platform_code` range at a time (see
/// [`partition_platform_ranges`]) and each range is streamed out as a Parquet row
/// group. Because [`renumber`] partitions by `platform_code`, renumbering each
/// range and emitting ranges in ascending order is data-identical to renumbering
/// the whole dataset at once; only the on-disk row-group layout differs. The range
/// budget is [`common::chunk_rows`] (`CTDDUMP_CHUNK_ROWS`).
///
/// Returns the number of input files merged.
pub fn run_concat_parquet(
    src_dir: &Path,
    output_file: &Path,
    config: &ConcatConfig,
) -> Result<usize, Box<dyn Error>> {
    let files = collect_parquet_files(src_dir, &config.pattern)?;
    let n = files.len();

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create output directory: {}", e))?;
        }
    }
    let out_file = std::fs::File::create(output_file)
        .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;

    // Zero-row frame from the first input defines the output schema for the batched
    // writer. When renumbering, run it through `renumber` so the schema matches the
    // real (renumbered) chunks exactly.
    let empty = LazyFrame::scan_parquet(&files[0], ScanArgsParquet::default())?
        .limit(0)
        .collect()?;
    let empty = if config.renumber {
        renumber(empty.lazy(), config.sort_by_pres).collect()?
    } else {
        empty
    };
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).batched(&schema)?;

    if !config.renumber {
        // No renumbering: stream each file straight through as its own row group.
        for path in &files {
            let df = LazyFrame::scan_parquet(path, ScanArgsParquet::default())?
                .collect()
                .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
            writer.write_batch(&df)?;
        }
        writer.finish()?;
        return Ok(n);
    }

    let index = scan_platform_index(&files)?;
    let ranges = partition_platform_ranges(&index.counts, common::chunk_rows() as u64);

    if ranges.is_empty() {
        // Every input was empty: still emit a valid, empty Parquet file.
        writer.write_batch(&empty)?;
        writer.finish()?;
        return Ok(n);
    }

    for (lo, hi) in ranges {
        // Assemble every row in this platform range, reading only files that overlap
        // it. The range is a contiguous slice of the distinct platform list, so this
        // filter selects exactly the range's platforms.
        let mut acc: Option<DataFrame> = None;
        for (path, file_min, file_max) in &index.spans {
            if file_max < &lo || file_min > &hi {
                continue;
            }
            let part = LazyFrame::scan_parquet(path, ScanArgsParquet::default())?
                .filter(
                    col("platform_code")
                        .gt_eq(lit(lo.as_str()))
                        .and(col("platform_code").lt_eq(lit(hi.as_str()))),
                )
                .collect()
                .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
            if part.height() == 0 {
                continue;
            }
            acc = Some(match acc {
                Some(a) => a.vstack(&part)?,
                None => part,
            });
        }

        let Some(frame) = acc else { continue };
        let result = renumber(frame.lazy(), config.sort_by_pres).collect()?;
        writer.write_batch(&result)?;
    }
    writer.finish()?;

    Ok(n)
}

/// Concatenate all YAML header files matching `pattern` under `src_dir` into a
/// single YAML file at `output_file`.
///
/// Each input file is expected to be a YAML mapping whose top-level keys are
/// file stems (as written by `ctddump header`).  The merged output is a single
/// mapping containing every key from every input file.  An error is returned
/// before writing if any two input files share the same top-level key.
///
/// Returns the number of input files merged.
pub fn run_concat_header(
    src_dir: &Path,
    output_file: &Path,
    pattern: &str,
) -> Result<usize, Box<dyn Error>> {
    let files = collect_parquet_files(src_dir, pattern)?;
    let n = files.len();

    let mut combined: HashMap<String, serde_yaml::Value> = HashMap::new();
    let mut duplicates: Vec<String> = Vec::new();

    for file in &files {
        let content = std::fs::read_to_string(file)
            .map_err(|e| format!("Cannot read {}: {}", file.display(), e))?;
        let map: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)
            .map_err(|e| format!("Cannot parse {}: {}", file.display(), e))?;
        for (key, value) in map {
            if combined.contains_key(&key) {
                duplicates.push(format!("  key '{}' in {}", key, file.display()));
            } else {
                combined.insert(key, value);
            }
        }
    }

    if !duplicates.is_empty() {
        return Err(format!(
            "Duplicate keys detected in YAML files:\n{}",
            duplicates.join("\n")
        )
        .into());
    }

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create output directory: {}", e))?;
        }
    }

    let out_file = std::fs::File::create(output_file)
        .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;
    serde_yaml::to_writer(out_file, &combined)?;

    Ok(n)
}
