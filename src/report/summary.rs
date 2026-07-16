//! `report summary`: assemble a multi-section Markdown or HTML page for one file
//! stem from the TSV reports produced by the pipeline.
//!
//! Every section's source file is auto-located under the standard pipeline layout
//! (`report/…` for the summary TSVs, `output/…` for the markdup duplicates TSV).
//! A section is emitted only when its file(s) exist, so a partially-run pipeline
//! still produces a valid page with just the available sections.
//!
//! The per-stage counts (platforms / profiles / observations) come from the
//! `report parquet --level platform` TSVs; the "% of original" / "Deleted" columns
//! compare each stage against the Conversion stage (or, if that is absent, the
//! earliest stage present). The Deduplication stages are additionally compared
//! against the *cleaned* counts — the last cleaning stage present — since that is
//! the input they actually ran on. The Mark-duplicates section reads the
//! `.dups.tsv` and splits duplicates into within-platform (a `dup_group` confined
//! to one platform) and across-platform (a group spanning two or more), and
//! tabulates how many profiles the duplicate groups hold.
//!
//! The page title and any notes are supplied by the caller (the pipeline scripts
//! pass a human-readable title and region- or product-specific notes); the section
//! prose is generic and applies to every region and dataset.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::SummaryFormat;

/// Distinct duplicate-group sizes are listed up to this many profiles per group;
/// larger groups collapse into one `<cap+1>+` row so the table stays bounded.
const GROUP_SIZE_CAP: u64 = 10;

/// Everything `run` needs besides the stem.
pub struct Opts<'a> {
    pub report_dir: &'a Path,
    pub out_dir: &'a Path,
    pub format: SummaryFormat,
    /// Page title; defaults to `Summary: <stem>`.
    pub title: Option<&'a str>,
    /// Free-text notes rendered under the page title.
    pub notes: &'a [String],
    pub output: Option<&'a Path>,
}

/// Assemble the summary page for `stem` and write it to `o.output` (or stdout).
pub fn run(stem: &str, o: Opts) -> Result<(), Box<dyn Error>> {
    let paths = StemPaths::new(stem, o.report_dir, o.out_dir);
    let sections = build_sections(&paths)?;
    let title = o.title.map_or_else(|| format!("Summary: {stem}"), str::to_string);

    let page = match o.format {
        SummaryFormat::Md => render_md(&title, o.notes, &sections),
        SummaryFormat::Html => render_html(&title, o.notes, &sections),
    };

    let mut w: Box<dyn Write> = match o.output {
        Some(p) => Box::new(
            File::create(p).map_err(|e| format!("Cannot create {}: {}", p.display(), e))?,
        ),
        None => Box::new(io::stdout().lock()),
    };
    w.write_all(page.as_bytes())?;
    w.flush()?;
    Ok(())
}

// ── Auto-located source files ─────────────────────────────────────────────────

/// The eight TSV files a summary can draw on, at their standard pipeline paths.
struct StemPaths {
    yaml: PathBuf,    // report/header/<stem>.yaml.tsv
    convert: PathBuf, // report/convert/<stem>.parquet.tsv
    dropqc: PathBuf,  // report/clean/dropqc/<stem>.parquet.tsv
    dropna: PathBuf,  // report/clean/dropna/<stem>.parquet.tsv
    filter: PathBuf,  // report/clean/filter/<stem>.parquet.tsv
    markdup: PathBuf, // report/dedup/markdup/<stem>.parquet.tsv
    dups: PathBuf,    // output/dedup/markdup/<stem>.dups.tsv
    dedup: PathBuf,   // report/dedup/dedup/<stem>.parquet.tsv
}

impl StemPaths {
    fn new(stem: &str, report_dir: &Path, out_dir: &Path) -> Self {
        let pq = |sub: &str| report_dir.join(sub).join(format!("{stem}.parquet.tsv"));
        StemPaths {
            yaml: report_dir.join("header").join(format!("{stem}.yaml.tsv")),
            convert: pq("convert"),
            dropqc: pq("clean/dropqc"),
            dropna: pq("clean/dropna"),
            filter: pq("clean/filter"),
            markdup: pq("dedup/markdup"),
            dups: out_dir.join("dedup/markdup").join(format!("{stem}.dups.tsv")),
            dedup: pq("dedup/dedup"),
        }
    }
}

// ── Minimal TSV reader ─────────────────────────────────────────────────────────

/// A parsed tab-separated file: a header plus string rows. The report TSVs are
/// small (one row per platform / per source file), so a plain std parse is enough
/// and avoids depending on a Polars CSV reader feature.
struct Tsv {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Tsv {
    fn read(path: &Path) -> Result<Tsv, Box<dyn Error>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        let mut lines = content.lines();
        let header = lines
            .next()
            .map(|l| l.split('\t').map(str::to_string).collect())
            .unwrap_or_default();
        let rows = lines
            .filter(|l| !l.is_empty())
            .map(|l| l.split('\t').map(str::to_string).collect())
            .collect();
        Ok(Tsv { header, rows })
    }

    fn col(&self, name: &str) -> Option<usize> {
        self.header.iter().position(|h| h == name)
    }

    /// Sum an integer-valued column across all rows (missing column → 0). Values
    /// are parsed leniently (a float is truncated) so a count column written by the
    /// report writer parses regardless of its exact formatting.
    fn sum_u64(&self, name: &str) -> u64 {
        let Some(i) = self.col(name) else { return 0 };
        self.rows.iter().map(|r| parse_u64(r.get(i))).sum()
    }

    /// Count rows whose column equals `val` (missing column → 0).
    fn count_eq(&self, name: &str, val: &str) -> u64 {
        let Some(i) = self.col(name) else { return 0 };
        self.rows.iter().filter(|r| r.get(i).map(String::as_str) == Some(val)).count() as u64
    }
}

fn parse_u64(cell: Option<&String>) -> u64 {
    let Some(s) = cell else { return 0 };
    let s = s.trim();
    s.parse::<u64>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|f| f.round() as u64))
        .unwrap_or(0)
}

// ── Counts and the intermediate page model ────────────────────────────────────

#[derive(Clone, Copy)]
struct Counts {
    platforms: u64,
    profiles: u64,
    obs: u64,
}

impl Counts {
    fn from_platform_tsv(t: &Tsv) -> Counts {
        Counts {
            platforms: t.rows.len() as u64,
            profiles: t.sum_u64("n_profiles"),
            obs: t.sum_u64("n_obs"),
        }
    }
}

struct Table {
    title: Option<String>,
    /// Optional prose under the table's title; empty for the unlabelled tables
    /// whose section description already explains them.
    desc: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

struct Section {
    level: u8, // 1 = `#`, 2 = `##`
    title: String,
    /// One or two generic sentences explaining what the stage did.
    desc: String,
    files: Vec<String>,
    tables: Vec<Table>,
}

// ── Section prose ─────────────────────────────────────────────────────────────
// Generic across every region and dataset; anything region- or product-specific
// belongs in a `--note` passed by the caller.

const DESC_FILES: &str = "Coverage of the core measurement variables and the two profile-level \
    QC flags across the source NetCDF files. A file counts as having one if the variable is \
    present, regardless of how many of its values are valid. \"Extra parameters\" lists the \
    non-core variables seen in any file.";

const DESC_CONVERT: &str = "Counts after converting the source NetCDF files to Parquet, before \
    any profile is dropped. These are the \"original\" numbers every later stage is compared against.";

const DESC_CLEANING: &str = "Stages that drop whole profiles that are unusable or out of scope. \
    They run in the order below, each on the output of the previous one, so the counts fall \
    monotonically.";

const DESC_DROPQC: &str = "Drops whole profiles whose profile-level quality flags are bad. \
    A profile is kept only if both its time and position QC are good (flag 1) or absent. Many \
    files ship no profile-level QC, and those profiles are kept.";

const DESC_DROPNA: &str = "Drops whole profiles that carry no valid measurement at all in any one \
    of temperature, salinity, or pressure. A profile is kept only if each of the three has at \
    least one valid observation.";

const DESC_FILTER: &str = "Keeps only the profiles whose position falls inside the region's \
    bounding box. A profile with a missing position counts as outside and is dropped.";

const DESC_DEDUP: &str = "Stages that find and remove profiles measuring the same time and place. \
    Profiles match on date and rounded longitude/latitude. The platform is not part of the key, \
    so duplicates are found across platforms as well as within one.";

const DESC_MARKDUP: &str = "Flags every profile that shares its (date, position) key with at least \
    one other. Nothing is removed here, so the counts match the previous stage; \"Duplicate \
    profiles\" is the pool the next stage picks a survivor from.";

const DESC_DUPS_WITHIN: &str = "Duplicate groups whose profiles all carry the same platform code: \
    one platform reporting the same cast more than once. Percentages are shares of all duplicate \
    profiles.";

const DESC_DUPS_ACROSS: &str = "Duplicate groups whose profiles carry two or more different \
    platform codes: the same cast reaching the archive under more than one platform, typically \
    via overlapping products. Percentages are shares of all duplicate profiles.";

const DESC_GROUP_SIZES: &str = "How many profiles each duplicate group holds. A group of 2 is a \
    cast recorded twice; larger groups are the same cast repeated more often. Sizes above 10 are \
    pooled into the final row.";

const DESC_REMOVE_DUPS: &str = "Keeps one profile per duplicate key and drops the rest. The \
    survivor is the profile with the most observations, with ties broken by first appearance. \
    Profiles with no key (missing time or position) are never treated as duplicates and \
    always survive.";

// ── Section assembly ──────────────────────────────────────────────────────────

fn build_sections(p: &StemPaths) -> Result<Vec<Section>, Box<dyn Error>> {
    let counts_of = |path: &Path| -> Result<Option<Counts>, Box<dyn Error>> {
        if path.exists() {
            Ok(Some(Counts::from_platform_tsv(&Tsv::read(path)?)))
        } else {
            Ok(None)
        }
    };

    // Baseline for the "% of original" / "Deleted" columns: the Conversion counts
    // if present, else the earliest stage that is.
    let mut baseline = None;
    for sp in [&p.convert, &p.dropqc, &p.dropna, &p.filter, &p.markdup, &p.dedup] {
        if let Some(c) = counts_of(sp)? {
            baseline = Some(c);
            break;
        }
    }

    // The deduplication stages ran on the cleaned data, so they are also compared
    // against the *last* cleaning stage present. If no cleaning stage ran, there is
    // nothing to compare against beyond the original and the columns are omitted.
    let mut cleaned = None;
    for sp in [&p.filter, &p.dropna, &p.dropqc] {
        if let Some(c) = counts_of(sp)? {
            cleaned = Some(c);
            break;
        }
    }

    let mut sections = Vec::new();

    // 1. File summary (from the header YAML report).
    if p.yaml.exists() {
        sections.push(file_summary_section(&Tsv::read(&p.yaml)?, path_str(&p.yaml)));
    }

    // 2. Conversion (baseline counts, no deletion columns).
    if let Some(c) = counts_of(&p.convert)? {
        sections.push(Section {
            level: 1,
            title: "Conversion".into(),
            desc: DESC_CONVERT.into(),
            files: vec![path_str(&p.convert)],
            tables: vec![counts_table(c)],
        });
    }

    // 3. Cleaning (Drop bad QC → Drop all-NA → Filter by region).
    let mut cleaning = Vec::new();
    for (title, desc, path) in [
        ("Drop bad QC", DESC_DROPQC, &p.dropqc),
        ("Drop all-NA profiles", DESC_DROPNA, &p.dropna),
        ("Filter by region", DESC_FILTER, &p.filter),
    ] {
        if let Some(c) = counts_of(path)? {
            cleaning.push(Section {
                level: 2,
                title: title.into(),
                desc: desc.into(),
                files: vec![path_str(path)],
                tables: vec![stage_table(c, baseline, None)],
            });
        }
    }
    if !cleaning.is_empty() {
        sections.push(parent_section("Cleaning", DESC_CLEANING));
        sections.extend(cleaning);
    }

    // 4. Deduplication (Mark duplicates → Remove duplicates).
    let mut dedup = Vec::new();
    if p.markdup.exists() {
        dedup.push(markdup_section(p, baseline, cleaned)?);
    }
    if let Some(c) = counts_of(&p.dedup)? {
        dedup.push(Section {
            level: 2,
            title: "Remove duplicates".into(),
            desc: DESC_REMOVE_DUPS.into(),
            files: vec![path_str(&p.dedup)],
            tables: vec![stage_table(c, baseline, cleaned)],
        });
    }
    if !dedup.is_empty() {
        sections.push(parent_section("Deduplication", DESC_DEDUP));
        sections.extend(dedup);
    }

    Ok(sections)
}

fn parent_section(title: &str, desc: &str) -> Section {
    Section { level: 1, title: title.into(), desc: desc.into(), files: vec![], tables: vec![] }
}

/// File-coverage summary from the header YAML report (one row per source file).
fn file_summary_section(t: &Tsv, path: String) -> Section {
    let n_files = t.rows.len() as u64;
    let mut rows = vec![vec!["Files".into(), fmt_int(n_files), String::new()]];
    for (label, colname) in [
        ("with TEMP", "has_temp"),
        ("with PSAL", "has_psal"),
        ("with PRES", "has_pres"),
        ("with DEPH", "has_deph"),
        ("with TIME_QC", "has_time_qc"),
        ("with POSITION_QC", "has_position_qc"),
    ] {
        let n = t.count_eq(colname, "true");
        rows.push(vec![label.into(), fmt_int(n), fmt_pct(pct(n, n_files))]);
    }

    // Distinct extra (non-core) parameters across all files.
    let mut extras: HashSet<String> = HashSet::new();
    if let Some(i) = t.col("extra_params") {
        for r in &t.rows {
            if let Some(cell) = r.get(i) {
                for p in cell.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    extras.insert(p.to_string());
                }
            }
        }
    }
    let mut extra_list: Vec<String> = extras.into_iter().collect();
    extra_list.sort();
    rows.push(vec![
        "Extra parameters".into(),
        if extra_list.is_empty() { "none".into() } else { extra_list.join(", ") },
        String::new(),
    ]);

    Section {
        level: 1,
        title: "File summary".into(),
        desc: DESC_FILES.into(),
        files: vec![path],
        tables: vec![Table {
            title: None,
            desc: String::new(),
            headers: vec!["Metric".into(), "Count".into(), "% of files".into()],
            rows,
        }],
    }
}

/// Mark-duplicates section: current counts + the duplicate profiles that would be
/// removed, plus the within-/across-platform split and the group-size distribution
/// from the `.dups.tsv`.
fn markdup_section(
    p: &StemPaths,
    baseline: Option<Counts>,
    cleaned: Option<Counts>,
) -> Result<Section, Box<dyn Error>> {
    let t = Tsv::read(&p.markdup)?;
    let c = Counts::from_platform_tsv(&t);
    let dup_profiles = t.sum_u64("dup_profiles");

    let mut files = vec![path_str(&p.markdup)];
    let mut tables = Vec::new();

    // Main table: counts vs original (and vs cleaned), plus the duplicate profiles
    // that the next stage would thin out. The extra row follows the same column
    // meanings — a count and its share of the original / cleaned profiles — but
    // leaves the "Deleted" cells blank, since this stage removes nothing.
    let mut main = stage_table(c, baseline, cleaned);
    let base_profiles = baseline.unwrap_or(c).profiles;
    let mut extra = vec![
        "Duplicate profiles".into(),
        fmt_int(dup_profiles),
        fmt_pct(pct(dup_profiles, base_profiles)),
        String::new(),
    ];
    if let Some(cl) = cleaned {
        extra.extend([
            fmt_int(cl.profiles),
            fmt_pct(pct(dup_profiles, cl.profiles)),
            String::new(),
        ]);
    }
    main.rows.push(extra);
    tables.push(main);

    // Within-/across-platform split and group sizes, from the .dups.tsv.
    if p.dups.exists() {
        files.push(path_str(&p.dups));
        let dups = Tsv::read(&p.dups)?;
        let groups = dup_groups(&dups);
        let (within, across) = dup_split(&groups);
        let total_profiles = within.profiles + across.profiles;
        let total_obs = within.obs + across.obs;
        tables.push(dup_table(
            "Duplicates within a platform",
            DESC_DUPS_WITHIN,
            &within,
            total_profiles,
            total_obs,
        ));
        tables.push(dup_table(
            "Duplicates across platforms",
            DESC_DUPS_ACROSS,
            &across,
            total_profiles,
            total_obs,
        ));
        tables.push(group_size_table(&groups, total_profiles));
    }

    Ok(Section {
        level: 2,
        title: "Mark duplicates".into(),
        desc: DESC_MARKDUP.into(),
        files,
        tables,
    })
}

/// How many profiles each duplicate group holds. Sizes above `GROUP_SIZE_CAP`
/// collapse into a single trailing `<cap+1>+` row.
fn group_size_table(groups: &[DupGroup], total_profiles: u64) -> Table {
    // BTreeMap keeps the sizes ascending; the cap maps every larger group to one bin.
    let mut bins: BTreeMap<u64, (u64, u64)> = BTreeMap::new(); // size → (groups, profiles)
    for g in groups {
        let e = bins.entry(g.profiles.min(GROUP_SIZE_CAP + 1)).or_default();
        e.0 += 1;
        e.1 += g.profiles;
    }

    let rows = bins
        .iter()
        .map(|(size, (n_groups, n_profiles))| {
            let label = if *size > GROUP_SIZE_CAP {
                format!("{}+", GROUP_SIZE_CAP + 1)
            } else {
                size.to_string()
            };
            vec![
                label,
                fmt_int(*n_groups),
                fmt_int(*n_profiles),
                fmt_pct(pct(*n_profiles, total_profiles)),
            ]
        })
        .collect();

    Table {
        title: Some("Duplicate group sizes".into()),
        desc: DESC_GROUP_SIZES.into(),
        headers: vec![
            "Profiles per group".into(),
            "Groups".into(),
            "Profiles".into(),
            "% of duplicates".into(),
        ],
        rows,
    }
}

#[derive(Default)]
struct DupStats {
    groups: u64,
    profiles: u64,
    obs: u64,
    platforms: HashSet<String>,
}

/// One `dup_group` of the `.dups.tsv`, aggregated.
#[derive(Default)]
struct DupGroup {
    platforms: HashSet<String>,
    profiles: u64,
    obs: u64,
}

/// Aggregate the `.dups.tsv` rows (one per duplicate profile) into their groups.
fn dup_groups(t: &Tsv) -> Vec<DupGroup> {
    let gi = t.col("dup_group");
    let pi = t.col("platform_code");
    let oi = t.col("n_obs");

    let mut groups: HashMap<String, DupGroup> = HashMap::new();
    for r in &t.rows {
        let g = gi.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let plat = pi.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let obs = oi.map(|i| parse_u64(r.get(i))).unwrap_or(0);
        let e = groups.entry(g).or_default();
        e.platforms.insert(plat);
        e.profiles += 1;
        e.obs += obs;
    }
    groups.into_values().collect()
}

/// Split the duplicate groups into within-platform (a group confined to a single
/// platform) and across-platform (spanning two or more).
fn dup_split(groups: &[DupGroup]) -> (DupStats, DupStats) {
    let mut within = DupStats::default();
    let mut across = DupStats::default();
    for g in groups {
        let target = if g.platforms.len() <= 1 { &mut within } else { &mut across };
        target.groups += 1;
        target.profiles += g.profiles;
        target.obs += g.obs;
        target.platforms.extend(g.platforms.iter().cloned());
    }
    (within, across)
}

fn dup_table(title: &str, desc: &str, s: &DupStats, total_profiles: u64, total_obs: u64) -> Table {
    Table {
        title: Some(title.into()),
        desc: desc.into(),
        headers: vec!["Metric".into(), "Count".into(), "% of duplicates".into()],
        rows: vec![
            vec!["Duplicate groups".into(), fmt_int(s.groups), String::new()],
            vec![
                "Duplicate profiles".into(),
                fmt_int(s.profiles),
                fmt_pct(pct(s.profiles, total_profiles)),
            ],
            vec!["Platforms".into(), fmt_int(s.platforms.len() as u64), String::new()],
            vec!["Observations".into(), fmt_int(s.obs), fmt_pct(pct(s.obs, total_obs))],
        ],
    }
}

// ── Count tables ──────────────────────────────────────────────────────────────

/// A plain platforms/profiles/observations table (used for the Conversion baseline).
fn counts_table(c: Counts) -> Table {
    Table {
        title: None,
        desc: String::new(),
        headers: vec!["Metric".into(), "Count".into()],
        rows: vec![
            vec!["Platforms".into(), fmt_int(c.platforms)],
            vec!["Profiles".into(), fmt_int(c.profiles)],
            vec!["Observations".into(), fmt_int(c.obs)],
        ],
    }
}

/// A stage table with "% of original" / "Deleted" columns relative to `baseline`.
///
/// When `cleaned` is given, three more columns compare the stage against the
/// cleaned data it actually ran on: the cleaned count, the stage's share of it,
/// and how much of it the stage deleted.
fn stage_table(c: Counts, baseline: Option<Counts>, cleaned: Option<Counts>) -> Table {
    let b = baseline.unwrap_or(c);

    let mut headers: Vec<String> =
        ["Metric", "Count", "% of original", "Deleted"].iter().map(|s| s.to_string()).collect();
    if cleaned.is_some() {
        headers.extend(
            ["Cleaned", "% of cleaned", "Deleted (cleaned)"].iter().map(|s| s.to_string()),
        );
    }

    let row = |name: &str, n: u64, base: u64, clean_base: Option<u64>| {
        let p = pct(n, base);
        let mut r = vec![name.into(), fmt_int(n), fmt_pct(p), fmt_pct(deleted(p))];
        if let Some(cb) = clean_base {
            let q = pct(n, cb);
            r.extend([fmt_int(cb), fmt_pct(q), fmt_pct(deleted(q))]);
        }
        r
    };

    Table {
        title: None,
        desc: String::new(),
        headers,
        rows: vec![
            row("Platforms", c.platforms, b.platforms, cleaned.map(|x| x.platforms)),
            row("Profiles", c.profiles, b.profiles, cleaned.map(|x| x.profiles)),
            row("Observations", c.obs, b.obs, cleaned.map(|x| x.obs)),
        ],
    }
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn pct(n: u64, base: u64) -> f64 {
    if base == 0 {
        f64::NAN
    } else {
        100.0 * n as f64 / base as f64
    }
}

fn deleted(pct_of_original: f64) -> f64 {
    if pct_of_original.is_nan() {
        f64::NAN
    } else {
        100.0 - pct_of_original
    }
}

fn fmt_pct(v: f64) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v:.1}%")
    }
}

/// Integer with thousands separators, e.g. `12345` → `12,345`.
fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

// ── Renderers ─────────────────────────────────────────────────────────────────

fn render_md(title: &str, notes: &[String], sections: &[Section]) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {title}\n\n"));
    for n in notes {
        s.push_str(&format!("> {n}\n\n"));
    }
    if sections.is_empty() {
        s.push_str("_No section source files were found._\n");
        return s;
    }
    for sec in sections {
        let hashes = "#".repeat(sec.level as usize + 1); // page title is `#`
        s.push_str(&format!("{hashes} {}\n\n", sec.title));
        if !sec.desc.is_empty() {
            s.push_str(&format!("{}\n\n", sec.desc));
        }
        for f in &sec.files {
            s.push_str(&format!("- File: `{f}`\n"));
        }
        if !sec.files.is_empty() {
            s.push('\n');
        }
        for t in &sec.tables {
            if let Some(title) = &t.title {
                s.push_str(&format!("**{title}**\n\n"));
            }
            if !t.desc.is_empty() {
                s.push_str(&format!("{}\n\n", t.desc));
            }
            s.push_str(&format!("| {} |\n", t.headers.join(" | ")));
            s.push_str(&format!("|{}|\n", t.headers.iter().map(|_| " --- ").collect::<Vec<_>>().join("|")));
            for row in &t.rows {
                s.push_str(&format!("| {} |\n", row.join(" | ")));
            }
            s.push('\n');
        }
    }
    s
}

fn render_html(title: &str, notes: &[String], sections: &[Section]) -> String {
    let mut b = String::new();
    b.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    b.push_str(&format!("<title>{}</title>\n", esc(title)));
    b.push_str("<style>\n");
    b.push_str(
        "body{font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;\
         max-width:60rem;margin:2rem auto;padding:0 1rem;color:#1a1a1a;line-height:1.5}\n\
         h1{border-bottom:2px solid #ccc;padding-bottom:.2em}\n\
         h2{margin-top:2rem;border-bottom:1px solid #e0e0e0;padding-bottom:.2em}\n\
         table{border-collapse:collapse;margin:.5rem 0 1.5rem}\n\
         th,td{border:1px solid #ccc;padding:.35rem .7rem;text-align:left}\n\
         th{background:#f2f2f2}\n\
         td:nth-child(n+2){text-align:right;font-variant-numeric:tabular-nums}\n\
         .files{color:#555;font-size:.9em;list-style:none;padding-left:0}\n\
         .files code{background:#f5f5f5;padding:.1em .3em;border-radius:3px}\n\
         .tabletitle{font-weight:600;margin:.5rem 0 .2rem}\n\
         .desc{margin:.5rem 0;max-width:48rem}\n\
         .note{border-left:4px solid #b0c4de;background:#f7fafd;margin:.6rem 0;\
         padding:.5rem .9rem}\n",
    );
    b.push_str("</style>\n</head>\n<body>\n");
    b.push_str(&format!("<h1>{}</h1>\n", esc(title)));
    for n in notes {
        b.push_str(&format!("<p class=\"note\">{}</p>\n", esc(n)));
    }

    if sections.is_empty() {
        b.push_str("<p><em>No section source files were found.</em></p>\n");
    }
    for sec in sections {
        // Shift one level below the page's <h1> title, mirroring the Markdown
        // renderer (page `#`, level-1 section `##`, level-2 `###`).
        let tag = format!("h{}", sec.level + 1);
        b.push_str(&format!("<{tag}>{}</{tag}>\n", esc(&sec.title)));
        if !sec.desc.is_empty() {
            b.push_str(&format!("<p class=\"desc\">{}</p>\n", esc(&sec.desc)));
        }
        if !sec.files.is_empty() {
            b.push_str("<ul class=\"files\">\n");
            for f in &sec.files {
                b.push_str(&format!("<li>File: <code>{}</code></li>\n", esc(f)));
            }
            b.push_str("</ul>\n");
        }
        for t in &sec.tables {
            if let Some(title) = &t.title {
                b.push_str(&format!("<p class=\"tabletitle\">{}</p>\n", esc(title)));
            }
            if !t.desc.is_empty() {
                b.push_str(&format!("<p class=\"desc\">{}</p>\n", esc(&t.desc)));
            }
            b.push_str("<table>\n<thead><tr>");
            for h in &t.headers {
                b.push_str(&format!("<th>{}</th>", esc(h)));
            }
            b.push_str("</tr></thead>\n<tbody>\n");
            for row in &t.rows {
                b.push_str("<tr>");
                for cell in row {
                    b.push_str(&format!("<td>{}</td>", esc(cell)));
                }
                b.push_str("</tr>\n");
            }
            b.push_str("</tbody>\n</table>\n");
        }
    }
    b.push_str("</body>\n</html>\n");
    b
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
