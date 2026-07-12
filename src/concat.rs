use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use glob::Pattern;
use polars::prelude::*;
use rayon::prelude::*;
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
    /// Drop rows whose `pres` is null or NaN before merging (default: `true`).
    ///
    /// Applied before renumbering, so `observation_no` stays contiguous
    /// (`1..N` over the remaining valid-pressure rows) rather than leaving gaps.
    /// Also honored in `--no-renumber` mode as a plain row filter. Set to `false`
    /// (via `--keep-na-pres`) to retain missing-pressure rows.
    pub drop_na_pres: bool,
    /// Number of range workers for the renumber path. `None` (the default) uses all
    /// logical CPU cores.
    ///
    /// When the effective count is `> 1`, platform ranges are renumbered in parallel,
    /// each writing a temporary Parquet file beside the output which are then
    /// concatenated in range order — trading higher peak memory
    /// (≈ `threads × CTDDUMP_CHUNK_ROWS` rows) and temporary disk for speed. `Some(1)`
    /// forces the sequential single-writer path (lowest memory; Polars still uses all
    /// cores within it). Ignored by the `--no-renumber` path.
    pub threads: Option<usize>,
}

impl Default for ConcatConfig {
    fn default() -> Self {
        ConcatConfig {
            pattern: "*.parquet".to_string(),
            renumber: true,
            sort_by_pres: true,
            drop_na_pres: true,
            threads: None,
        }
    }
}

/// Drop rows whose `pres` is null or NaN.
fn filter_valid_pres(lf: LazyFrame) -> LazyFrame {
    lf.filter(col("pres").is_not_null().and(col("pres").is_not_nan()))
}

/// Temporary Parquet path for range `i`, placed beside the final output. The
/// suffix has no `.parquet` extension so a later `*.parquet` scan never picks up a
/// stray temp file left behind by a crash.
fn temp_path_for(output: &Path, i: usize) -> PathBuf {
    let name = output
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("concat_output");
    output.with_file_name(format!("{}.concat-tmp-{:05}", name, i))
}

/// Assemble one platform range `[lo, hi]` from the overlapping input files, drop
/// missing-`pres` rows if configured, and renumber. Returns `None` when the range
/// has no rows (before or after dropping). The range is a contiguous slice of the
/// distinct platform list, so the `platform_code` filter selects exactly its
/// platforms — and because `renumber` partitions by `platform_code`, each range is
/// a fully independent unit of work.
fn build_range_df(
    spans: &[(PathBuf, String, String)],
    lo: &str,
    hi: &str,
    config: &ConcatConfig,
) -> Result<Option<DataFrame>, Box<dyn Error>> {
    let mut acc: Option<DataFrame> = None;
    for (path, file_min, file_max) in spans {
        if file_max.as_str() < lo || file_min.as_str() > hi {
            continue;
        }
        let part = LazyFrame::scan_parquet(path, ScanArgsParquet::default())?
            .filter(
                col("platform_code")
                    .gt_eq(lit(lo))
                    .and(col("platform_code").lt_eq(lit(hi))),
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

    let Some(frame) = acc else { return Ok(None) };
    let mut lf = frame.lazy();
    if config.drop_na_pres {
        lf = filter_valid_pres(lf);
    }
    let df = renumber(lf, config.sort_by_pres).collect()?;
    if df.height() == 0 {
        return Ok(None);
    }
    Ok(Some(df))
}

/// Build one range and write it to `temp_path` (nothing is written for an empty
/// range). Errors are returned as `String` so the result is `Send` for rayon.
fn build_and_write_range(
    spans: &[(PathBuf, String, String)],
    lo: &str,
    hi: &str,
    config: &ConcatConfig,
    temp_path: &Path,
) -> Result<(), String> {
    match build_range_df(spans, lo, hi, config).map_err(|e| e.to_string())? {
        None => Ok(()),
        Some(mut df) => {
            let f = std::fs::File::create(temp_path)
                .map_err(|e| format!("Cannot create temp file {}: {}", temp_path.display(), e))?;
            ParquetWriter::new(f)
                .set_parallel(false)
                .finish(&mut df)
                .map_err(|e| e.to_string())?;
            Ok(())
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

    // An empty result is not an error; the callers report it and write no output.
    // Deterministic order across platforms.
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
/// [`partition_platform_ranges`]). Because [`renumber`] partitions by
/// `platform_code`, renumbering each range and emitting ranges in ascending order
/// is data-identical to renumbering the whole dataset at once; only the on-disk
/// row-group layout differs. The range budget is [`common::chunk_rows`]
/// (`CTDDUMP_CHUNK_ROWS`).
///
/// When more than one range worker is used — `config.threads` is `None` (all cores,
/// the default) or `Some(n > 1)` — the renumber path runs the per-range work in
/// parallel: each range is written to a temporary Parquet file beside the output,
/// then the temp files are concatenated in range order into the final output (and
/// removed). Ranges are fully independent (each owns complete platforms), so the
/// merged result is identical to the sequential path — only faster, at the cost of
/// ≈ `threads × CTDDUMP_CHUNK_ROWS` peak rows and temporary disk. `config.threads =
/// Some(1)` forces the sequential single-writer path.
///
/// Returns the number of input files merged.
pub fn run_concat_parquet(
    src_dir: &Path,
    output_file: &Path,
    config: &ConcatConfig,
) -> Result<usize, Box<dyn Error>> {
    let files = collect_parquet_files(src_dir, &config.pattern)?;
    let n = files.len();

    // No inputs is not an error: report it and write no output, so a workflow can
    // reference a not-yet-available dataset (e.g. Baltic GL) without failing.
    if files.is_empty() {
        eprintln!(
            "No files matching '{}' under {}; no output written.",
            config.pattern,
            src_dir.display()
        );
        return Ok(0);
    }

    // Number of range workers: an explicit `--threads N`, or all logical cores when
    // omitted. `n_threads == 1` uses the sequential single-writer path (and lets
    // Polars parallelize that one stream); `n_threads > 1` renumbers ranges in
    // parallel. In the parallel case, pin Polars to one internal thread so
    // `--threads` is the real knob (see batch::run_batch) — otherwise N range
    // workers each spawn Polars' own N_cpus pool on top. This must run before any
    // Polars call, since Polars reads the var once when it lazily inits its pool.
    let n_threads = match config.threads {
        Some(n) => n.max(1),
        None => std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    };
    let parallel = n_threads > 1;
    if parallel && std::env::var_os("POLARS_MAX_THREADS").is_none() {
        std::env::set_var("POLARS_MAX_THREADS", "1");
    }

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create output directory: {}", e))?;
        }
    }

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

    if !config.renumber {
        // No renumbering: stream each file straight through as its own row group.
        let out_file = std::fs::File::create(output_file)
            .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;
        let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;
        for path in &files {
            let mut lf = LazyFrame::scan_parquet(path, ScanArgsParquet::default())?;
            if config.drop_na_pres {
                lf = filter_valid_pres(lf);
            }
            let df = lf
                .collect()
                .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
            writer.write_batch(&df)?;
        }
        writer.finish()?;
        return Ok(n);
    }

    let index = scan_platform_index(&files)?;
    let ranges = partition_platform_ranges(&index.counts, common::chunk_rows() as u64);

    // Parallel path: renumber ranges concurrently into temp files, then concatenate
    // the temp files in range order.
    if parallel {
        if !ranges.is_empty() {
            let temp_paths: Vec<PathBuf> =
                (0..ranges.len()).map(|i| temp_path_for(output_file, i)).collect();

            let work = || -> Vec<String> {
                ranges
                    .par_iter()
                    .zip(temp_paths.par_iter())
                    .filter_map(|((lo, hi), tmp)| {
                        build_and_write_range(&index.spans, lo, hi, config, tmp).err()
                    })
                    .collect()
            };

            // rayon's default 2 MiB worker stack overflows inside Polars' parquet
            // writer on large ranges; match the 16 MiB stack used by batch mode.
            const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;
            let errors = rayon::ThreadPoolBuilder::new()
                .stack_size(WORKER_STACK_SIZE)
                .num_threads(n_threads)
                .build()
                .map_err(|e| format!("Failed to build thread pool: {}", e))?
                .install(work);

            if !errors.is_empty() {
                for tmp in &temp_paths {
                    let _ = std::fs::remove_file(tmp);
                }
                return Err(format!("Concat processing errors:\n{}", errors.join("\n")).into());
            }

            // Concatenate the per-range temp files in ascending range order.
            let out_file = std::fs::File::create(output_file)
                .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;
            let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;
            let mut wrote_any = false;
            for tmp in &temp_paths {
                if !tmp.exists() {
                    continue; // range produced no rows
                }
                let df = LazyFrame::scan_parquet(tmp, ScanArgsParquet::default())?
                    .collect()
                    .map_err(|e| format!("Cannot read temp file {}: {}", tmp.display(), e))?;
                if df.height() > 0 {
                    writer.write_batch(&df)?;
                    wrote_any = true;
                }
            }
            if !wrote_any {
                writer.write_batch(&empty)?;
            }
            writer.finish()?;

            for tmp in &temp_paths {
                let _ = std::fs::remove_file(tmp);
            }
            return Ok(n);
        }
        // No ranges: fall through to the sequential empty-output handling below.
    }

    // Sequential path: one writer, ranges assembled and written in ascending order.
    let out_file = std::fs::File::create(output_file)
        .map_err(|e| format!("Cannot create {}: {}", output_file.display(), e))?;
    let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;

    if ranges.is_empty() {
        // Every input was empty: still emit a valid, empty Parquet file.
        writer.write_batch(&empty)?;
        writer.finish()?;
        return Ok(n);
    }

    for (lo, hi) in &ranges {
        if let Some(df) = build_range_df(&index.spans, lo, hi, config)? {
            writer.write_batch(&df)?;
        }
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

    // No inputs is not an error: report it and write no output (see run_concat_parquet).
    if files.is_empty() {
        eprintln!(
            "No files matching '{}' under {}; no output written.",
            pattern,
            src_dir.display()
        );
        return Ok(0);
    }

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
