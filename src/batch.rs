use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use glob::Pattern;
use rayon::prelude::*;
use walkdir::WalkDir;

/// Recursively collect all files under `src_dir` whose filename matches `pattern`.
/// `pattern` is a glob matched against the filename only (not the full path).
/// An empty result is **not** an error — the caller decides what to do (see
/// [`run_batch`], which treats it as "nothing to do").
pub fn collect_nc_files(src_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let glob_pat = Pattern::new(pattern)
        .map_err(|e| format!("Invalid pattern '{}': {}", pattern, e))?;

    let files: Vec<PathBuf> = WalkDir::new(src_dir)
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

    Ok(files)
}

/// Compute `(src, dest)` pairs.
/// If `output_dir` is given, all outputs land flat in that directory.
/// Otherwise each output is placed alongside its source.
/// `output_ext` is the file extension for output files (e.g. `"parquet"` or `"yaml"`).
pub fn compute_output_pairs(
    nc_files: &[PathBuf],
    output_dir: Option<&Path>,
    output_ext: &str,
) -> Result<Vec<(PathBuf, PathBuf)>, Box<dyn Error>> {
    nc_files
        .iter()
        .map(|src| {
            let stem = src
                .file_stem()
                .ok_or_else(|| format!("Cannot get file stem for {}", src.display()))?;
            let dest = match output_dir {
                Some(dir) => dir.join(stem).with_extension(output_ext),
                None => src.with_extension(output_ext),
            };
            Ok((src.clone(), dest))
        })
        .collect()
}

/// Error if any two sources would produce the same output path.
pub fn check_duplicates(pairs: &[(PathBuf, PathBuf)]) -> Result<(), Box<dyn Error>> {
    let mut seen: HashMap<&PathBuf, &PathBuf> = HashMap::new();
    let mut duplicates: Vec<String> = Vec::new();

    for (src, dest) in pairs {
        if let Some(existing_src) = seen.insert(dest, src) {
            duplicates.push(format!(
                "  {} and {} both map to {}",
                existing_src.display(),
                src.display(),
                dest.display()
            ));
        }
    }

    if !duplicates.is_empty() {
        return Err(
            format!("Duplicate output filenames detected:\n{}", duplicates.join("\n")).into(),
        );
    }

    Ok(())
}

/// Walk `src_dir`, select files matching `pattern` (filename glob), process each
/// using `process_fn`, and write results to the same directory (if `output_dir`
/// is `None`) or flat into `output_dir`.
/// `output_ext` controls the extension of output files (`"parquet"` or `"yaml"`).
///
/// Returns the number of files successfully processed.
/// Errors from individual files are collected and returned together at the end.
pub fn run_batch<F>(
    src_dir: &Path,
    output_dir: Option<&Path>,
    threads: Option<usize>,
    output_ext: &str,
    pattern: &str,
    process_fn: F,
) -> Result<usize, Box<dyn Error>>
where
    F: Fn(&str, &str) -> Result<(), Box<dyn Error>> + Sync + Send,
{
    let nc_files = collect_nc_files(src_dir, pattern)?;

    // An empty match is not an error: report it and produce nothing. This lets a
    // workflow reference a dataset that is not (yet) available — e.g. the Global
    // (GL) product for the Baltic Sea — without failing the run.
    if nc_files.is_empty() {
        eprintln!(
            "No files matching '{}' under {}; nothing to do.",
            pattern,
            src_dir.display()
        );
        return Ok(0);
    }

    let pairs = compute_output_pairs(&nc_files, output_dir, output_ext)?;
    check_duplicates(&pairs)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create output directory {}: {}", dir.display(), e))?;
    }

    // Parallelism here is per file. Each conversion also calls Polars, which has
    // its own global thread pool sized to the logical CPU count — so without this
    // cap the process spawns `threads` + N_cpus workers, far exceeding `--threads`.
    // Pin Polars to a single internal thread so `--threads` is the real knob.
    // Polars reads this when it lazily initializes its pool (first call in the
    // parallel loop below), so setting it here is early enough. Respect a value
    // the user set explicitly.
    if std::env::var_os("POLARS_MAX_THREADS").is_none() {
        std::env::set_var("POLARS_MAX_THREADS", "1");
    }

    let run = || -> Vec<String> {
        pairs
            .par_iter()
            .filter_map(|(src, dest)| {
                process_fn(
                    src.to_str().unwrap_or_default(),
                    dest.to_str().unwrap_or_default(),
                )
                .err()
                .map(|e| format!("{}: {}", src.display(), e))
            })
            .collect()
    };

    // Build an explicit pool with a generous worker stack. rayon's default 2 MiB
    // worker stack overflows on large files inside Polars' parquet writer; this
    // matches the stack raised via RUST_MIN_STACK in `main` regardless of env
    // timing. When `threads` is None we still use a controlled pool (default
    // thread count) so both paths get the larger stack.
    const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;
    let mut builder = rayon::ThreadPoolBuilder::new().stack_size(WORKER_STACK_SIZE);
    if let Some(n) = threads {
        builder = builder.num_threads(n);
    }
    let errors = builder
        .build()
        .map_err(|e| format!("Failed to build thread pool: {}", e))?
        .install(run);

    if !errors.is_empty() {
        return Err(format!("Batch processing errors:\n{}", errors.join("\n")).into());
    }

    Ok(pairs.len())
}

#[cfg(test)]
mod tests {
    use super::{check_duplicates, compute_output_pairs};
    use std::path::PathBuf;

    // ── compute_output_pairs ─────────────────────────────────────────────────

    #[test]
    fn test_output_pairs_flat_into_dir() {
        let files = vec![
            PathBuf::from("/data/a/file1.nc"),
            PathBuf::from("/data/b/file2.nc"),
        ];
        let out = PathBuf::from("/out");
        let pairs = compute_output_pairs(&files, Some(&out), "parquet").unwrap();
        assert_eq!(pairs[0], (PathBuf::from("/data/a/file1.nc"), PathBuf::from("/out/file1.parquet")));
        assert_eq!(pairs[1], (PathBuf::from("/data/b/file2.nc"), PathBuf::from("/out/file2.parquet")));
    }

    #[test]
    fn test_output_pairs_in_place() {
        let files = vec![PathBuf::from("/data/sub/file.nc")];
        let pairs = compute_output_pairs(&files, None, "parquet").unwrap();
        assert_eq!(pairs[0].1, PathBuf::from("/data/sub/file.parquet"));
    }

    #[test]
    fn test_output_pairs_yaml_extension() {
        let files = vec![PathBuf::from("/data/file.nc")];
        let pairs = compute_output_pairs(&files, None, "yaml").unwrap();
        assert_eq!(pairs[0].1, PathBuf::from("/data/file.yaml"));
    }

    #[test]
    fn test_output_pairs_preserves_stem_with_hyphens() {
        // Oceanographic filenames like AR_PR_CT_ITP-71 contain hyphens and underscores
        let files = vec![PathBuf::from("/data/AR_PR_CT_ITP-71.nc")];
        let pairs = compute_output_pairs(&files, None, "parquet").unwrap();
        assert_eq!(pairs[0].1, PathBuf::from("/data/AR_PR_CT_ITP-71.parquet"));
    }

    #[test]
    fn test_output_pairs_empty_input() {
        let pairs = compute_output_pairs(&[], None, "parquet").unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_output_pairs_src_preserved() {
        // The src path in the pair must equal the original input
        let f = PathBuf::from("/data/file.nc");
        let pairs = compute_output_pairs(&[f.clone()], None, "parquet").unwrap();
        assert_eq!(pairs[0].0, f);
    }

    // ── check_duplicates ─────────────────────────────────────────────────────

    #[test]
    fn test_check_duplicates_no_conflict() {
        let pairs = vec![
            (PathBuf::from("/a/f1.nc"), PathBuf::from("/out/f1.parquet")),
            (PathBuf::from("/b/f2.nc"), PathBuf::from("/out/f2.parquet")),
        ];
        assert!(check_duplicates(&pairs).is_ok());
    }

    #[test]
    fn test_check_duplicates_detects_conflict() {
        let pairs = vec![
            (PathBuf::from("/a/file.nc"), PathBuf::from("/out/file.parquet")),
            (PathBuf::from("/b/file.nc"), PathBuf::from("/out/file.parquet")),
        ];
        let err = check_duplicates(&pairs).unwrap_err().to_string();
        assert!(err.contains("Duplicate output filenames detected"), "got: {err}");
    }

    #[test]
    fn test_check_duplicates_error_names_conflicting_sources() {
        let pairs = vec![
            (PathBuf::from("/a/file.nc"), PathBuf::from("/out/file.parquet")),
            (PathBuf::from("/b/file.nc"), PathBuf::from("/out/file.parquet")),
        ];
        let err = check_duplicates(&pairs).unwrap_err().to_string();
        // Both source paths should appear in the error so the user knows which files clash
        assert!(err.contains("file.nc"), "got: {err}");
    }

    #[test]
    fn test_check_duplicates_empty() {
        assert!(check_duplicates(&[]).is_ok());
    }

    #[test]
    fn test_check_duplicates_single_file() {
        let pairs = vec![(PathBuf::from("/a/f.nc"), PathBuf::from("/out/f.parquet"))];
        assert!(check_duplicates(&pairs).is_ok());
    }
}
