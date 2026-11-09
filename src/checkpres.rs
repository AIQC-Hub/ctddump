//! Find profiles whose pressure does not increase in observation order.
//!
//! Meant for the per-file Parquet written by `convert` / `batch convert`, before
//! `concat convert` sorts every profile by pressure: there, rows are still in the
//! source file's order, so a decreasing step is the source's own. A profile is
//! one `(filename, platform_code, profile_no)`, which is exact before the merge
//! and keeps profiles from different files apart after it.
//!
//! Within a profile, each observation with a pressure is compared with the
//! previous one that had a pressure (missing pressures are skipped). A step down
//! by more than `min_drop` is a violation; a repeated pressure is not. A
//! violating profile is `reversed` when no step goes up (stored bottom-up) and
//! `non_monotonic` otherwise. A profile with no usable pressure is not checked
//! and is counted apart (`n_no_pres`), so the counts match what `concat` keeps.
//!
//! Files are read one at a time in fixed row windows and processed in parallel;
//! only per-profile records are kept, so memory follows the profile count of one
//! file plus the violating profiles. Three TSVs are written: the violating
//! profiles, the platforms that have any, and one row of totals.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use polars::prelude::*;
use rayon::prelude::*;

use crate::concat::collect_parquet_files;
use crate::convert::common;

/// Options for [`run`].
#[derive(Debug, Clone)]
pub struct Opts {
    /// Filename glob used when the source is a directory.
    pub pattern: String,
    /// Decreases no larger than this (dbar) are ignored.
    pub min_drop: f64,
    /// Only pressures whose `pres_qc` is one of these flags are used; empty uses
    /// every pressure.
    pub pres_qc: Vec<String>,
    /// Worker threads (files in parallel); `None` uses all cores.
    pub threads: Option<usize>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { pattern: "*.parquet".to_string(), min_drop: 0.0, pres_qc: Vec::new(), threads: None }
    }
}

/// How a violating profile's pressure misbehaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Every changing step goes down: the profile is stored bottom-up.
    Reversed,
    /// Pressure goes both up and down.
    NonMonotonic,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Reversed => "reversed",
            Kind::NonMonotonic => "non_monotonic",
        }
    }
}

/// Running state for one profile while its rows are read.
struct ProfState {
    filename: String,
    platform_code: String,
    profile_no: u32,
    ts_ms: Option<i64>,
    profile_time: f64,
    lon: f64,
    lat: f64,
    n_obs: u64,
    n_pres: u64,
    n_up: u64,
    n_down: u64,
    first_decrease_obs: Option<u32>,
    max_decrease: f64,
    prev: Option<f32>,
    first: Option<f32>,
    last: f32,
    min: f32,
    max: f32,
    any_conv: bool,
}

impl ProfState {
    fn kind(&self) -> Option<Kind> {
        match (self.n_down, self.n_up) {
            (0, _) => None,
            (_, 0) => Some(Kind::Reversed),
            _ => Some(Kind::NonMonotonic),
        }
    }
}

/// One row of the profiles TSV.
struct BadProfile {
    filename: String,
    platform_code: String,
    profile_no: u32,
    ts_ms: Option<i64>,
    profile_time: f64,
    lon: f64,
    lat: f64,
    kind: Kind,
    n_obs: u64,
    n_pres: u64,
    n_decreases: u64,
    first_decrease_obs: u32,
    max_decrease: f64,
    pres_first: f32,
    pres_last: f32,
    pres_min: f32,
    pres_max: f32,
    pres_conv: bool,
}

/// Per-platform counts.
#[derive(Default)]
struct PlatAgg {
    n_files: u64,
    n_profiles: u64,
    n_bad: u64,
    n_reversed: u64,
    n_non_monotonic: u64,
    first_bad: Option<i64>,
    last_bad: Option<i64>,
}

impl PlatAgg {
    fn add_bad(&mut self, kind: Kind, ts_ms: Option<i64>) {
        self.n_bad += 1;
        match kind {
            Kind::Reversed => self.n_reversed += 1,
            Kind::NonMonotonic => self.n_non_monotonic += 1,
        }
        if let Some(t) = ts_ms {
            self.first_bad = Some(self.first_bad.map_or(t, |f| f.min(t)));
            self.last_bad = Some(self.last_bad.map_or(t, |l| l.max(t)));
        }
    }

    fn merge(&mut self, o: &PlatAgg) {
        self.n_files += o.n_files;
        self.n_profiles += o.n_profiles;
        self.n_bad += o.n_bad;
        self.n_reversed += o.n_reversed;
        self.n_non_monotonic += o.n_non_monotonic;
        for t in [o.first_bad, o.last_bad].into_iter().flatten() {
            self.first_bad = Some(self.first_bad.map_or(t, |f| f.min(t)));
            self.last_bad = Some(self.last_bad.map_or(t, |l| l.max(t)));
        }
    }
}

/// What one file contributes. Profiles without any usable pressure are only
/// counted in `n_no_pres`: there is nothing to check, and `concat` drops them.
struct FileResult {
    n_profiles: u64,
    n_no_pres: u64,
    bad: Vec<BadProfile>,
    platforms: HashMap<String, PlatAgg>,
}

/// Totals for the whole source (the summary TSV row).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    pub n_files: u64,
    pub n_platforms: u64,
    /// Profiles with at least one usable pressure (the ones checked).
    pub n_profiles: u64,
    /// Profiles with no usable pressure, not checked.
    pub n_no_pres: u64,
    pub n_bad_profiles: u64,
    pub n_reversed: u64,
    pub n_non_monotonic: u64,
    pub n_bad_platforms: u64,
    pub n_bad_files: u64,
}

impl Totals {
    pub fn pct_bad(&self) -> f64 {
        if self.n_profiles == 0 {
            0.0
        } else {
            100.0 * self.n_bad_profiles as f64 / self.n_profiles as f64
        }
    }
}

/// Check every Parquet file under `src` (or `src` itself when it is a file) and
/// write `<out_prefix>.profiles.tsv`, `.platforms.tsv` and `.summary.tsv`.
/// Returns `None`, writing nothing, when no file matches.
pub fn run(src: &Path, out_prefix: &Path, opts: &Opts) -> Result<Option<Totals>, Box<dyn Error>> {
    if opts.min_drop.is_nan() || opts.min_drop < 0.0 {
        return Err(format!("--min-drop must be zero or positive, got {}", opts.min_drop).into());
    }
    // An absent directory matches nothing, as in `concat`: a no-op, not an error.
    let files: Vec<PathBuf> =
        if src.is_file() { vec![src.to_path_buf()] } else { collect_parquet_files(src, &opts.pattern)? };
    if files.is_empty() {
        eprintln!("No files matching '{}' under {}; nothing to do.", opts.pattern, src.display());
        return Ok(None);
    }

    // As in `batch`: parallelism is per file, so keep Polars to one internal
    // thread unless the user set it, or each worker would start a full pool.
    if std::env::var_os("POLARS_MAX_THREADS").is_none() {
        std::env::set_var("POLARS_MAX_THREADS", "1");
    }
    const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;
    let mut builder = rayon::ThreadPoolBuilder::new().stack_size(WORKER_STACK_SIZE);
    if let Some(n) = opts.threads {
        builder = builder.num_threads(n);
    }
    let results: Vec<Result<FileResult, String>> = builder
        .build()
        .map_err(|e| format!("Failed to build thread pool: {}", e))?
        .install(|| {
            files
                .par_iter()
                .with_max_len(1)
                .map(|f| check_file(f, opts).map_err(|e| format!("{}: {}", f.display(), e)))
                .collect()
        });

    let mut errors = Vec::new();
    let mut totals = Totals { n_files: files.len() as u64, ..Totals::default() };
    let mut bad: Vec<BadProfile> = Vec::new();
    let mut platforms: BTreeMap<String, PlatAgg> = BTreeMap::new();
    let mut bad_files: HashSet<String> = HashSet::new();
    for r in results {
        match r {
            Err(e) => errors.push(e),
            Ok(fr) => {
                totals.n_profiles += fr.n_profiles;
                totals.n_no_pres += fr.n_no_pres;
                for (code, agg) in &fr.platforms {
                    platforms.entry(code.clone()).or_default().merge(agg);
                }
                if !fr.bad.is_empty() {
                    bad_files.extend(fr.bad.iter().map(|b| b.filename.clone()));
                }
                bad.extend(fr.bad);
            }
        }
    }
    if !errors.is_empty() {
        return Err(format!("Pressure check errors:\n{}", errors.join("\n")).into());
    }

    totals.n_platforms = platforms.len() as u64;
    totals.n_bad_profiles = bad.len() as u64;
    totals.n_reversed = bad.iter().filter(|b| b.kind == Kind::Reversed).count() as u64;
    totals.n_non_monotonic = totals.n_bad_profiles - totals.n_reversed;
    totals.n_bad_platforms = platforms.values().filter(|p| p.n_bad > 0).count() as u64;
    totals.n_bad_files = bad_files.len() as u64;

    bad.sort_by(|a, b| {
        (&a.platform_code, a.ts_ms, &a.filename, a.profile_no).cmp(&(&b.platform_code, b.ts_ms, &b.filename, b.profile_no))
    });

    if let Some(dir) = out_prefix.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create output directory {}: {}", dir.display(), e))?;
    }
    write_profiles(&with_suffix(out_prefix, ".profiles.tsv"), &bad)?;
    write_platforms(&with_suffix(out_prefix, ".platforms.tsv"), &platforms)?;
    write_summary(&with_suffix(out_prefix, ".summary.tsv"), &totals)?;

    eprintln!(
        "{} of {} profiles ({:.2}%) have decreasing pressure: {} reversed, {} non-monotonic; {} without pressure not checked",
        totals.n_bad_profiles,
        totals.n_profiles,
        totals.pct_bad(),
        totals.n_reversed,
        totals.n_non_monotonic,
        totals.n_no_pres
    );
    Ok(Some(totals))
}

/// `<prefix><suffix>`, keeping any dots already in the prefix.
fn with_suffix(prefix: &Path, suffix: &str) -> PathBuf {
    let mut s: OsString = prefix.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// `col(name)` cast to `dtype`, or a typed null column when the file lacks it.
fn opt_col(schema: &Schema, name: &str, dtype: DataType) -> Expr {
    if schema.contains(name) {
        col(name).cast(dtype)
    } else {
        lit(NULL).cast(dtype).alias(name)
    }
}

fn check_file(path: &Path, opts: &Opts) -> Result<FileResult, Box<dyn Error>> {
    let scan = || {
        LazyFrame::scan_parquet(path, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", path.display(), e))
    };
    let schema = scan()?.slice(0, 0).collect()?.schema();
    for required in ["platform_code", "profile_no", "pres"] {
        if !schema.contains(required) {
            return Err(format!("missing column `{required}`").into());
        }
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let filename = if schema.contains("filename") {
        col("filename").cast(DataType::String)
    } else {
        lit(stem.as_str()).alias("filename")
    };
    let exprs = [
        filename,
        col("platform_code").cast(DataType::String),
        col("profile_no").cast(DataType::UInt32),
        col("pres").cast(DataType::Float32),
        opt_col(&schema, "observation_no", DataType::UInt32),
        opt_col(&schema, "profile_time", DataType::Float64),
        opt_col(&schema, "profile_timestamp", DataType::Int64),
        opt_col(&schema, "longitude", DataType::Float64),
        opt_col(&schema, "latitude", DataType::Float64),
        opt_col(&schema, "pres_qc", DataType::String),
        opt_col(&schema, "pres_conv", DataType::Int8),
    ];

    let total = scan()?.select([len().alias("n")]).collect()?.column("n")?.u32()?.get(0).unwrap_or(0) as usize;

    let mut profs: Vec<ProfState> = Vec::new();
    let mut index: HashMap<(String, String, u32), usize> = HashMap::new();
    let mut last: Option<usize> = None;
    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?.slice(offset as i64, count as IdxSize).select(exprs.clone()).collect()?;
        let fname = df.column("filename")?.str()?;
        let pc = df.column("platform_code")?.str()?;
        let pn = df.column("profile_no")?.u32()?;
        let pres = df.column("pres")?.f32()?;
        let obs = df.column("observation_no")?.u32()?;
        let pt = df.column("profile_time")?.f64()?;
        let ts = df.column("profile_timestamp")?.i64()?;
        let lon = df.column("longitude")?.f64()?;
        let lat = df.column("latitude")?.f64()?;
        let qc = df.column("pres_qc")?.str()?;
        let conv = df.column("pres_conv")?.i8()?;

        for i in 0..df.height() {
            let (f, p, n) = (fname.get(i).unwrap_or(""), pc.get(i).unwrap_or(""), pn.get(i).unwrap_or(0));
            // Rows of a profile are normally contiguous: reuse the last index.
            let idx = match last {
                Some(j) if profs[j].profile_no == n && profs[j].platform_code == p && profs[j].filename == f => j,
                _ => *index.entry((f.to_string(), p.to_string(), n)).or_insert_with(|| {
                    profs.push(ProfState {
                        filename: f.to_string(),
                        platform_code: p.to_string(),
                        profile_no: n,
                        ts_ms: ts.get(i),
                        profile_time: pt.get(i).unwrap_or(f64::NAN),
                        lon: lon.get(i).unwrap_or(f64::NAN),
                        lat: lat.get(i).unwrap_or(f64::NAN),
                        n_obs: 0,
                        n_pres: 0,
                        n_up: 0,
                        n_down: 0,
                        first_decrease_obs: None,
                        max_decrease: 0.0,
                        prev: None,
                        first: None,
                        last: f32::NAN,
                        min: f32::INFINITY,
                        max: f32::NEG_INFINITY,
                        any_conv: false,
                    });
                    profs.len() - 1
                }),
            };
            last = Some(idx);
            let st = &mut profs[idx];
            st.n_obs += 1;
            if conv.get(i) == Some(1) {
                st.any_conv = true;
            }
            let Some(v) = pres.get(i).filter(|v| v.is_finite()) else { continue };
            if !opts.pres_qc.is_empty() && !opts.pres_qc.iter().any(|q| Some(q.as_str()) == qc.get(i)) {
                continue;
            }
            st.n_pres += 1;
            st.first.get_or_insert(v);
            st.last = v;
            st.min = st.min.min(v);
            st.max = st.max.max(v);
            if let Some(prev) = st.prev {
                if v > prev {
                    st.n_up += 1;
                } else if v < prev {
                    let drop = prev as f64 - v as f64;
                    if drop > opts.min_drop {
                        st.n_down += 1;
                        st.first_decrease_obs.get_or_insert(obs.get(i).unwrap_or(st.n_obs as u32));
                        st.max_decrease = st.max_decrease.max(drop);
                    }
                }
            }
            st.prev = Some(v);
        }
        offset += count;
    }

    let mut platforms: HashMap<String, PlatAgg> = HashMap::new();
    let mut bad = Vec::new();
    let mut n_no_pres = 0u64;
    for st in &profs {
        if st.n_pres == 0 {
            n_no_pres += 1;
            continue;
        }
        let agg = platforms.entry(st.platform_code.clone()).or_default();
        agg.n_profiles += 1;
        if let Some(kind) = st.kind() {
            agg.add_bad(kind, st.ts_ms);
            bad.push(BadProfile {
                filename: st.filename.clone(),
                platform_code: st.platform_code.clone(),
                profile_no: st.profile_no,
                ts_ms: st.ts_ms,
                profile_time: st.profile_time,
                lon: st.lon,
                lat: st.lat,
                kind,
                n_obs: st.n_obs,
                n_pres: st.n_pres,
                n_decreases: st.n_down,
                first_decrease_obs: st.first_decrease_obs.unwrap_or(0),
                max_decrease: st.max_decrease,
                pres_first: st.first.unwrap_or(f32::NAN),
                pres_last: st.last,
                pres_min: st.min,
                pres_max: st.max,
                pres_conv: st.any_conv,
            });
        }
    }
    for agg in platforms.values_mut() {
        agg.n_files = 1;
    }
    Ok(FileResult { n_profiles: profs.len() as u64 - n_no_pres, n_no_pres, bad, platforms })
}

fn fmt_ts(ts_ms: Option<i64>) -> String {
    ts_ms.and_then(DateTime::from_timestamp_millis).map(|dt| dt.naive_utc().to_string()).unwrap_or_default()
}

fn fmt_date(ts_ms: Option<i64>) -> String {
    ts_ms
        .and_then(DateTime::from_timestamp_millis)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn write_profiles(path: &Path, rows: &[BadProfile]) -> Result<(), Box<dyn Error>> {
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(path).map_err(|e| format!("Cannot create {}: {}", path.display(), e))?,
    );
    writeln!(
        f,
        "filename\tplatform_code\tprofile_no\tprofile_timestamp\tprofile_time\tlongitude\tlatitude\tkind\t\
         n_obs\tn_pres\tn_decreases\tfirst_decrease_obs\tmax_decrease\tpres_first\tpres_last\tpres_min\tpres_max\tpres_conv"
    )?;
    for r in rows {
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
            r.filename,
            r.platform_code,
            r.profile_no,
            fmt_ts(r.ts_ms),
            r.profile_time,
            r.lon,
            r.lat,
            r.kind.as_str(),
            r.n_obs,
            r.n_pres,
            r.n_decreases,
            r.first_decrease_obs,
            r.max_decrease,
            r.pres_first,
            r.pres_last,
            r.pres_min,
            r.pres_max,
            u8::from(r.pres_conv),
        )?;
    }
    f.flush()?;
    Ok(())
}

fn write_platforms(path: &Path, platforms: &BTreeMap<String, PlatAgg>) -> Result<(), Box<dyn Error>> {
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(path).map_err(|e| format!("Cannot create {}: {}", path.display(), e))?,
    );
    writeln!(f, "platform_code\tn_files\tn_profiles\tn_bad_profiles\tpct_bad\tn_reversed\tn_non_monotonic\tfirst_date\tlast_date")?;
    for (code, p) in platforms.iter().filter(|(_, p)| p.n_bad > 0) {
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{:.2}\t{}\t{}\t{}\t{}",
            code,
            p.n_files,
            p.n_profiles,
            p.n_bad,
            100.0 * p.n_bad as f64 / p.n_profiles as f64,
            p.n_reversed,
            p.n_non_monotonic,
            fmt_date(p.first_bad),
            fmt_date(p.last_bad),
        )?;
    }
    f.flush()?;
    Ok(())
}

fn write_summary(path: &Path, t: &Totals) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(path).map_err(|e| format!("Cannot create {}: {}", path.display(), e))?;
    writeln!(
        f,
        "n_files\tn_platforms\tn_profiles\tn_no_pres\tn_bad_profiles\tpct_bad\tn_reversed\tn_non_monotonic\tn_bad_platforms\tn_bad_files"
    )?;
    writeln!(
        f,
        "{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{}\t{}\t{}",
        t.n_files,
        t.n_platforms,
        t.n_profiles,
        t.n_no_pres,
        t.n_bad_profiles,
        t.pct_bad(),
        t.n_reversed,
        t.n_non_monotonic,
        t.n_bad_platforms,
        t.n_bad_files
    )?;
    Ok(())
}
