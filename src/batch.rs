use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

/// Recursively collect all `.nc` files under `src_dir`.
pub fn collect_nc_files(src_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let files: Vec<PathBuf> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "nc"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if files.is_empty() {
        return Err(format!("No .nc files found under {}", src_dir.display()).into());
    }

    Ok(files)
}

/// Compute `(src, dest)` pairs.
/// If `output_dir` is given, all outputs land flat in that directory.
/// Otherwise each output is placed alongside its source (`.nc` → `.parquet`).
pub fn compute_output_pairs(
    nc_files: &[PathBuf],
    output_dir: Option<&Path>,
) -> Result<Vec<(PathBuf, PathBuf)>, Box<dyn Error>> {
    nc_files
        .iter()
        .map(|src| {
            let stem = src
                .file_stem()
                .ok_or_else(|| format!("Cannot get file stem for {}", src.display()))?;
            let dest = match output_dir {
                Some(dir) => dir.join(stem).with_extension("parquet"),
                None => src.with_extension("parquet"),
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

/// Walk `src_dir`, convert every `.nc` file using `convert_fn`, and write results to
/// the same directory (if `output_dir` is `None`) or flat into `output_dir`.
///
/// Returns the number of files successfully converted.
/// Errors from individual files are collected and returned together at the end.
pub fn run_batch<F>(
    src_dir: &Path,
    output_dir: Option<&Path>,
    threads: Option<usize>,
    convert_fn: F,
) -> Result<usize, Box<dyn Error>>
where
    F: Fn(&str, &str) -> Result<(), Box<dyn Error>> + Sync + Send,
{
    let nc_files = collect_nc_files(src_dir)?;
    let pairs = compute_output_pairs(&nc_files, output_dir)?;
    check_duplicates(&pairs)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create output directory {}: {}", dir.display(), e))?;
    }

    let run = || -> Vec<String> {
        pairs
            .par_iter()
            .filter_map(|(src, dest)| {
                convert_fn(
                    src.to_str().unwrap_or_default(),
                    dest.to_str().unwrap_or_default(),
                )
                .err()
                .map(|e| format!("{}: {}", src.display(), e))
            })
            .collect()
    };

    let errors = if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build()
            .map_err(|e| format!("Failed to build thread pool: {}", e))?
            .install(run)
    } else {
        run()
    };

    if !errors.is_empty() {
        return Err(format!("Batch conversion errors:\n{}", errors.join("\n")).into());
    }

    Ok(pairs.len())
}
