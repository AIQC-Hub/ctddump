//! Integration tests for `report summary`. Fixtures are the small TSV files the
//! pipeline's `report` steps produce, written in-test (no external data needed).

use std::fs;
use std::path::Path;

use ctddump::handle_dispatch;

fn dispatch(args: &[&str]) {
    handle_dispatch(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .expect("report summary should succeed");
}

/// Write a platform-level parquet report TSV. `rows` are (platform, profiles, obs)
/// triples; `dup` optionally adds a `dup_profiles` column (one value per row).
fn write_platform_tsv(path: &Path, rows: &[(&str, u64, u64)], dup: Option<&[u64]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut s = String::from("platform_code\tn_profiles\tn_obs\ttime_qc_good");
    if dup.is_some() {
        s.push_str("\tdup_profiles");
    }
    s.push('\n');
    for (i, (p, prof, obs)) in rows.iter().enumerate() {
        s.push_str(&format!("{p}\t{prof}\t{obs}\t0"));
        if let Some(d) = dup {
            s.push_str(&format!("\t{}", d[i]));
        }
        s.push('\n');
    }
    fs::write(path, s).unwrap();
}

fn write_yaml_tsv(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        "filename\thas_temp\thas_psal\thas_pres\thas_deph\thas_time\thas_position\textra_params\n\
         AR001\ttrue\ttrue\ttrue\tfalse\ttrue\ttrue\tDOXY;FLU2\n\
         AR002\ttrue\tfalse\ttrue\tfalse\ttrue\ttrue\tDOXY\n",
    )
    .unwrap();
}

/// dups.tsv: group 1 within (AR001×2), group 2 across (AR001,AR002), group 3
/// within (AR001×3). → within: 2 groups / 5 profiles; across: 1 group / 2 profiles.
fn write_dups_tsv(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut s = String::from(
        "dup_group\tplatform_code\tprofile_no\tprofile_time\tprofile_timestamp\tlongitude\tlatitude\tn_obs\n",
    );
    for (g, p, no) in [
        (1, "AR001", 5),
        (1, "AR001", 6),
        (2, "AR001", 7),
        (2, "AR002", 3),
        (3, "AR001", 8),
        (3, "AR001", 9),
        (3, "AR001", 10),
    ] {
        s.push_str(&format!("{g}\t{p}\t{no}\t0\t0\t0\t0\t100\n"));
    }
    fs::write(path, s).unwrap();
}

/// Build the full report/output tree for `stem` and return (report_dir, out_dir).
fn full_tree(root: &Path, stem: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let rep = root.join("report");
    let out = root.join("output");
    write_yaml_tsv(&rep.join("header").join(format!("{stem}.yaml.tsv")));
    write_platform_tsv(
        &rep.join("convert").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 100, 10000), ("AR002", 100, 10000)],
        None,
    );
    write_platform_tsv(
        &rep.join("clean/dropqc").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 95, 9500), ("AR002", 100, 10000)],
        None,
    );
    write_platform_tsv(
        &rep.join("clean/dropna").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 90, 9000), ("AR002", 95, 9500)],
        None,
    );
    write_platform_tsv(
        &rep.join("clean/filter").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 80, 8000)],
        None,
    );
    write_platform_tsv(
        &rep.join("dedup/markdup").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 80, 8000)],
        Some(&[7]),
    );
    write_dups_tsv(&out.join("dedup/markdup").join(format!("{stem}.dups.tsv")));
    write_platform_tsv(
        &rep.join("dedup/dedup").join(format!("{stem}.parquet.tsv")),
        &[("AR001", 73, 7300)],
        None,
    );
    (rep, out)
}

fn run_summary(rep: &Path, out: &Path, stem: &str, format: &str, dest: &Path) -> String {
    dispatch(&[
        "report", "summary", stem,
        "--report-dir", rep.to_str().unwrap(),
        "--out-dir", out.to_str().unwrap(),
        "--format", format,
        "-o", dest.to_str().unwrap(),
    ]);
    fs::read_to_string(dest).unwrap()
}

#[test]
fn markdown_has_all_sections_and_correct_percentages() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    let md = run_summary(&rep, &out, "nrt_ar_ar", "md", &dir.path().join("s.md"));

    // All seven sections present (parent headings + subsections).
    for h in [
        "## File summary",
        "## Conversion",
        "## Cleaning",
        "### Drop bad QC",
        "### Drop all-NA profiles",
        "### Filter by region",
        "## Deduplication",
        "### Mark duplicates",
        "### Remove duplicates",
    ] {
        assert!(md.contains(h), "missing heading: {h}\n{md}");
    }

    // Baseline = Conversion (200 profiles / 20000 obs). Filter keeps 80 profiles.
    assert!(md.contains("| Profiles | 80 | 40.0% | 60.0% |"), "filter profile % wrong\n{md}");
    // Drop QC: 195/200 profiles = 97.5%.
    assert!(md.contains("| Profiles | 195 | 97.5% | 2.5% |"), "dropqc profile % wrong\n{md}");
    // Remove duplicates: 73/200 = 36.5%.
    assert!(md.contains("| Profiles | 73 | 36.5% | 63.5% |"), "dedup profile % wrong\n{md}");

    // File coverage: PSAL present in 1/2 files.
    assert!(md.contains("| with PSAL | 1 | 50.0% |"), "psal coverage wrong\n{md}");
    assert!(md.contains("| Extra parameters | DOXY, FLU2 |"), "extras wrong\n{md}");

    // Within/across split: within 5 profiles (71.4%), across 2 profiles (28.6%, 2 platforms).
    assert!(md.contains("**Duplicates within a platform**"), "no within table\n{md}");
    assert!(md.contains("| Duplicate profiles | 5 | 71.4% |"), "within profiles wrong\n{md}");
    assert!(md.contains("**Duplicates across platforms**"), "no across table\n{md}");
    assert!(md.contains("| Duplicate profiles | 2 | 28.6% |"), "across profiles wrong\n{md}");
    assert!(md.contains("| Platforms | 2 |"), "across platforms wrong\n{md}");
}

#[test]
fn html_is_self_contained_and_escaped() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    let html = run_summary(&rep, &out, "nrt_ar_ar", "html", &dir.path().join("s.html"));

    assert!(html.starts_with("<!DOCTYPE html>"), "not an HTML doc");
    assert!(html.contains("<style>") && html.contains("</html>"), "not self-contained");
    // One <table> per data table: File(1) + Conversion(1) + 3 cleaning + markdup(3) + dedup(1) = 9.
    assert_eq!(html.matches("<table>").count(), 9, "unexpected table count");
    assert!(html.contains("<h2>File summary</h2>"), "missing section heading");
}

#[test]
fn missing_files_skip_their_sections() {
    let dir = tempfile::tempdir().unwrap();
    let rep = dir.path().join("report");
    let out = dir.path().join("output");
    // Only Conversion and Filter present — nothing else.
    write_platform_tsv(&rep.join("convert").join("part.parquet.tsv"), &[("P1", 100, 5000)], None);
    write_platform_tsv(&rep.join("clean/filter").join("part.parquet.tsv"), &[("P1", 60, 3000)], None);

    let md = run_summary(&rep, &out, "part", "md", &dir.path().join("p.md"));

    assert!(md.contains("## Conversion"), "conversion missing\n{md}");
    assert!(md.contains("## Cleaning") && md.contains("### Filter by region"), "filter missing\n{md}");
    // Absent sources → no section.
    assert!(!md.contains("File summary"), "yaml section should be skipped\n{md}");
    assert!(!md.contains("Drop bad QC"), "dropqc section should be skipped\n{md}");
    assert!(!md.contains("Deduplication"), "dedup section should be skipped\n{md}");
    // Baseline falls back to Conversion: Filter 60/100 = 60.0%.
    assert!(md.contains("| Profiles | 60 | 60.0% | 40.0% |"), "filter % wrong\n{md}");
}

#[test]
fn no_files_produces_valid_empty_page() {
    let dir = tempfile::tempdir().unwrap();
    let md = run_summary(
        &dir.path().join("nope-report"),
        &dir.path().join("nope-output"),
        "ghost",
        "md",
        &dir.path().join("empty.md"),
    );
    assert!(md.contains("# Summary: ghost"), "missing title\n{md}");
    assert!(md.contains("No section source files were found"), "missing empty note\n{md}");
}
