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
//! earliest stage present). The Mark-duplicates section additionally reads the
//! `.dups.tsv` and splits duplicates into within-platform (a `dup_group` confined
//! to one platform) and across-platform (a group spanning two or more).

use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::SummaryFormat;

/// Assemble the summary page for `stem` and write it to `output` (or stdout).
pub fn run(
    stem: &str,
    report_dir: &Path,
    out_dir: &Path,
    format: SummaryFormat,
    output: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let paths = StemPaths::new(stem, report_dir, out_dir);
    let sections = build_sections(stem, &paths)?;

    let page = match format {
        SummaryFormat::Md => render_md(stem, &sections),
        SummaryFormat::Html => render_html(stem, &sections),
    };

    let mut w: Box<dyn Write> = match output {
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
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

struct Section {
    level: u8, // 1 = `#`, 2 = `##`
    title: String,
    files: Vec<String>,
    tables: Vec<Table>,
}

// ── Section assembly ──────────────────────────────────────────────────────────

fn build_sections(_stem: &str, p: &StemPaths) -> Result<Vec<Section>, Box<dyn Error>> {
    // Baseline for the "% of original" / "Deleted" columns: the Conversion counts
    // if present, else the earliest stage that is.
    let stage_paths = [&p.convert, &p.dropqc, &p.dropna, &p.filter, &p.markdup, &p.dedup];
    let mut baseline = None;
    for sp in stage_paths {
        if sp.exists() {
            baseline = Some(Counts::from_platform_tsv(&Tsv::read(sp)?));
            break;
        }
    }

    let mut sections = Vec::new();

    // 1. File summary (from the header YAML report).
    if p.yaml.exists() {
        sections.push(file_summary_section(&Tsv::read(&p.yaml)?, path_str(&p.yaml)));
    }

    // 2. Conversion (baseline counts, no deletion columns).
    if p.convert.exists() {
        let c = Counts::from_platform_tsv(&Tsv::read(&p.convert)?);
        sections.push(Section {
            level: 1,
            title: "Conversion".into(),
            files: vec![path_str(&p.convert)],
            tables: vec![counts_table(c)],
        });
    }

    // 3. Cleaning (Drop bad QC → Drop all-NA → Filter by region).
    let mut cleaning = Vec::new();
    for (title, path) in [
        ("Drop bad QC", &p.dropqc),
        ("Drop all-NA profiles", &p.dropna),
        ("Filter by region", &p.filter),
    ] {
        if path.exists() {
            let c = Counts::from_platform_tsv(&Tsv::read(path)?);
            cleaning.push(Section {
                level: 2,
                title: title.into(),
                files: vec![path_str(path)],
                tables: vec![stage_table(c, baseline)],
            });
        }
    }
    if !cleaning.is_empty() {
        sections.push(parent_section("Cleaning"));
        sections.extend(cleaning);
    }

    // 4. Deduplication (Mark duplicates → Remove duplicates).
    let mut dedup = Vec::new();
    if p.markdup.exists() {
        dedup.push(markdup_section(p, baseline)?);
    }
    if p.dedup.exists() {
        let c = Counts::from_platform_tsv(&Tsv::read(&p.dedup)?);
        dedup.push(Section {
            level: 2,
            title: "Remove duplicates".into(),
            files: vec![path_str(&p.dedup)],
            tables: vec![stage_table(c, baseline)],
        });
    }
    if !dedup.is_empty() {
        sections.push(parent_section("Deduplication"));
        sections.extend(dedup);
    }

    Ok(sections)
}

fn parent_section(title: &str) -> Section {
    Section { level: 1, title: title.into(), files: vec![], tables: vec![] }
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
        ("with TIME", "has_time"),
        ("with POSITION_QC", "has_position"),
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
        if extra_list.is_empty() { "—".into() } else { extra_list.join(", ") },
        String::new(),
    ]);

    Section {
        level: 1,
        title: "File summary".into(),
        files: vec![path],
        tables: vec![Table {
            title: None,
            headers: vec!["Metric".into(), "Count".into(), "% of files".into()],
            rows,
        }],
    }
}

/// Mark-duplicates section: current counts + the duplicate profiles that would be
/// removed, plus the within-/across-platform split from the `.dups.tsv`.
fn markdup_section(p: &StemPaths, baseline: Option<Counts>) -> Result<Section, Box<dyn Error>> {
    let t = Tsv::read(&p.markdup)?;
    let c = Counts::from_platform_tsv(&t);
    let dup_profiles = t.sum_u64("dup_profiles");

    let mut files = vec![path_str(&p.markdup)];
    let mut tables = Vec::new();

    // Main table: counts vs original, plus duplicate profiles (would be removed).
    let mut main = stage_table(c, baseline);
    main.rows.push(vec![
        "Duplicate profiles".into(),
        fmt_int(dup_profiles),
        String::new(),
        fmt_pct(pct(dup_profiles, c.profiles)), // share of current profiles
    ]);
    tables.push(main);

    // Within-/across-platform duplicate tables from the .dups.tsv.
    if p.dups.exists() {
        files.push(path_str(&p.dups));
        let dups = Tsv::read(&p.dups)?;
        let (within, across) = dup_split(&dups);
        let total_profiles = within.profiles + across.profiles;
        let total_obs = within.obs + across.obs;
        tables.push(dup_table("Duplicates within a platform", &within, total_profiles, total_obs));
        tables.push(dup_table("Duplicates across platforms", &across, total_profiles, total_obs));
    }

    Ok(Section { level: 2, title: "Mark duplicates".into(), files, tables })
}

#[derive(Default)]
struct DupStats {
    groups: u64,
    profiles: u64,
    obs: u64,
    platforms: HashSet<String>,
}

/// Split the duplicate profiles into within-platform (a `dup_group` confined to a
/// single platform) and across-platform (spanning two or more).
fn dup_split(t: &Tsv) -> (DupStats, DupStats) {
    let gi = t.col("dup_group");
    let pi = t.col("platform_code");
    let oi = t.col("n_obs");

    // Aggregate per dup_group: its platform set, profile count, observation count.
    let mut groups: HashMap<String, (HashSet<String>, u64, u64)> = HashMap::new();
    for r in &t.rows {
        let g = gi.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let plat = pi.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let obs = oi.map(|i| parse_u64(r.get(i))).unwrap_or(0);
        let e = groups.entry(g).or_default();
        e.0.insert(plat);
        e.1 += 1;
        e.2 += obs;
    }

    let mut within = DupStats::default();
    let mut across = DupStats::default();
    for (_g, (plats, profiles, obs)) in groups {
        let target = if plats.len() <= 1 { &mut within } else { &mut across };
        target.groups += 1;
        target.profiles += profiles;
        target.obs += obs;
        target.platforms.extend(plats);
    }
    (within, across)
}

fn dup_table(title: &str, s: &DupStats, total_profiles: u64, total_obs: u64) -> Table {
    Table {
        title: Some(title.into()),
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
        headers: vec!["Metric".into(), "Count".into()],
        rows: vec![
            vec!["Platforms".into(), fmt_int(c.platforms)],
            vec!["Profiles".into(), fmt_int(c.profiles)],
            vec!["Observations".into(), fmt_int(c.obs)],
        ],
    }
}

/// A stage table with "% of original" and "Deleted" columns relative to `baseline`.
fn stage_table(c: Counts, baseline: Option<Counts>) -> Table {
    let row = |name: &str, n: u64, base: u64| {
        let p = pct(n, base);
        vec![name.into(), fmt_int(n), fmt_pct(p), fmt_pct(deleted(p))]
    };
    let b = baseline.unwrap_or(c);
    Table {
        title: None,
        headers: vec![
            "Metric".into(),
            "Count".into(),
            "% of original".into(),
            "Deleted".into(),
        ],
        rows: vec![
            row("Platforms", c.platforms, b.platforms),
            row("Profiles", c.profiles, b.profiles),
            row("Observations", c.obs, b.obs),
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

fn render_md(stem: &str, sections: &[Section]) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Summary: {stem}\n\n"));
    if sections.is_empty() {
        s.push_str("_No section source files were found._\n");
        return s;
    }
    for sec in sections {
        let hashes = "#".repeat(sec.level as usize + 1); // page title is `#`
        s.push_str(&format!("{hashes} {}\n\n", sec.title));
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

fn render_html(stem: &str, sections: &[Section]) -> String {
    let mut b = String::new();
    b.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    b.push_str(&format!("<title>Summary: {}</title>\n", esc(stem)));
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
         .tabletitle{font-weight:600;margin:.5rem 0 .2rem}\n",
    );
    b.push_str("</style>\n</head>\n<body>\n");
    b.push_str(&format!("<h1>Summary: {}</h1>\n", esc(stem)));

    if sections.is_empty() {
        b.push_str("<p><em>No section source files were found.</em></p>\n");
    }
    for sec in sections {
        // Shift one level below the page's <h1> title, mirroring the Markdown
        // renderer (page `#`, level-1 section `##`, level-2 `###`).
        let tag = format!("h{}", sec.level + 1);
        b.push_str(&format!("<{tag}>{}</{tag}>\n", esc(&sec.title)));
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
