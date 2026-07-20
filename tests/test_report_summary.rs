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
        "filename\thas_temp\thas_psal\thas_pres\thas_deph\thas_time_qc\thas_position_qc\textra_params\n\
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
    run_summary_with(rep, out, stem, format, dest, &[])
}

/// `extra` appends further CLI arguments (e.g. `--title`, `--note`).
fn run_summary_with(
    rep: &Path,
    out: &Path,
    stem: &str,
    format: &str,
    dest: &Path,
    extra: &[&str],
) -> String {
    let mut args = vec![
        "report", "summary", stem,
        "--report-dir", rep.to_str().unwrap(),
        "--out-dir", out.to_str().unwrap(),
        "--format", format,
        "-o", dest.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    dispatch(&args);
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
    // Remove duplicates: 73/200 = 36.5% of original, and against the cleaned data
    // (Filter, the last cleaning stage = 80 profiles) 73/80 = 91.25%, which formats
    // half-to-even as 91.2%.
    assert!(
        md.contains("| Profiles | 73 | 36.5% | 63.5% | 80 | 91.2% | 8.8% |"),
        "dedup profile % wrong\n{md}"
    );
    // The cleaned columns are a deduplication-only addition.
    assert!(md.contains("| Metric | Count | % of original | Deleted |"), "cleaning cols wrong\n{md}");
    assert!(
        md.contains("| Metric | Count | % of original | Deleted | Cleaned | % of cleaned | Deleted (cleaned) |"),
        "dedup cols wrong\n{md}"
    );

    // Section prose is present and generic.
    assert!(md.contains("bounding box"), "filter description missing\n{md}");

    // File coverage: PSAL present in 1/2 files.
    assert!(md.contains("| with PSAL | 1 | 50.0% |"), "psal coverage wrong\n{md}");
    assert!(md.contains("| Extra parameters | DOXY, FLU2 |"), "extras wrong\n{md}");

    // Within/across split: within 5 profiles (71.4%), across 2 profiles (28.6%, 2 platforms).
    assert!(md.contains("**Duplicates within a platform**"), "no within table\n{md}");
    assert!(md.contains("| Duplicate profiles | 5 | 71.4% |"), "within profiles wrong\n{md}");
    assert!(md.contains("**Duplicates across platforms**"), "no across table\n{md}");
    assert!(md.contains("| Duplicate profiles | 2 | 28.6% |"), "across profiles wrong\n{md}");
    assert!(md.contains("| Platforms | 2 |"), "across platforms wrong\n{md}");

    // The markdup "Duplicate profiles" row shares the table's columns: 7 profiles
    // are 3.5% of the original 200 and 8.75% (→ 8.8%) of the cleaned 80. Nothing is
    // deleted at this stage, so both "Deleted" cells stay blank.
    assert!(
        md.contains("| Duplicate profiles | 7 | 3.5% |  | 80 | 8.8% |  |"),
        "markdup duplicate row wrong\n{md}"
    );

    // Each labelled duplicate table carries its own short explanation.
    assert!(md.contains("all carry the same platform code"), "within desc missing\n{md}");
    assert!(md.contains("two or more different platform codes"), "across desc missing\n{md}");
    assert!(md.contains("A group of 2 is a cast recorded twice"), "group-size desc missing\n{md}");

    // Group sizes: two groups of 2 (4 profiles, 57.1%), one group of 3 (42.9%).
    assert!(md.contains("**Duplicate group sizes**"), "no group-size table\n{md}");
    assert!(md.contains("| 2 | 2 | 4 | 57.1% |"), "size-2 row wrong\n{md}");
    assert!(md.contains("| 3 | 1 | 3 | 42.9% |"), "size-3 row wrong\n{md}");
}

#[test]
fn title_and_notes_are_used() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    let md = run_summary_with(
        &rep, &out, "nrt_ar_ar", "md", &dir.path().join("t.md"),
        &["--title", "NRT Arctic: regional (AR)", "--note", "First note.", "--note", "Second <note>."],
    );
    assert!(md.starts_with("# NRT Arctic: regional (AR)\n"), "custom title missing\n{md}");
    assert!(!md.contains("Summary: nrt_ar_ar"), "default title should be replaced\n{md}");
    assert!(md.contains("> First note.") && md.contains("> Second <note>."), "notes missing\n{md}");

    let html = run_summary_with(
        &rep, &out, "nrt_ar_ar", "html", &dir.path().join("t.html"),
        &["--title", "NRT Arctic", "--note", "Second <note>."],
    );
    assert!(html.contains("<title>NRT Arctic</title>"), "html title missing\n{html}");
    assert!(html.contains("<h1>NRT Arctic</h1>"), "html heading missing\n{html}");
    // Notes are caller-supplied text, so they must be escaped, not injected.
    assert!(html.contains("Second &lt;note&gt;."), "note not escaped\n{html}");
}

/// Groups larger than the cap collapse into one `11+` row.
#[test]
fn large_duplicate_groups_collapse_into_a_tail_row() {
    let dir = tempfile::tempdir().unwrap();
    let rep = dir.path().join("report");
    let out = dir.path().join("output");
    write_platform_tsv(
        &rep.join("dedup/markdup").join("big.parquet.tsv"),
        &[("P1", 30, 3000)],
        Some(&[30]),
    );
    // One group of 12 profiles, one group of 2.
    let mut s = String::from(
        "dup_group\tplatform_code\tprofile_no\tprofile_time\tprofile_timestamp\tlongitude\tlatitude\tn_obs\n",
    );
    for i in 0..12 {
        s.push_str(&format!("1\tP1\t{i}\t0\t0\t0\t0\t10\n"));
    }
    for i in 0..2 {
        s.push_str(&format!("2\tP1\t{}\t0\t0\t0\t0\t10\n", 100 + i));
    }
    fs::create_dir_all(out.join("dedup/markdup")).unwrap();
    fs::write(out.join("dedup/markdup").join("big.dups.tsv"), s).unwrap();

    let md = run_summary(&rep, &out, "big", "md", &dir.path().join("big.md"));

    // 12 > cap(10) → the `11+` bin; 12/14 = 85.7%. The group of 2 stays its own row.
    assert!(md.contains("| 2 | 1 | 2 | 14.3% |"), "size-2 row wrong\n{md}");
    assert!(md.contains("| 11+ | 1 | 12 | 85.7% |"), "tail row wrong\n{md}");
    // No cleaning stage ran, so the dedup table has no cleaned columns.
    assert!(!md.contains("% of cleaned"), "cleaned cols should be absent\n{md}");
}

#[test]
fn html_is_self_contained_and_escaped() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    let html = run_summary(&rep, &out, "nrt_ar_ar", "html", &dir.path().join("s.html"));

    assert!(html.starts_with("<!DOCTYPE html>"), "not an HTML doc");
    assert!(html.contains("<style>") && html.contains("</html>"), "not self-contained");
    // One <table> per data table: File(1) + Conversion(1) + 3 cleaning + markdup(4) + dedup(1) = 10.
    assert_eq!(html.matches("<table>").count(), 10, "unexpected table count");
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

/// House style: no em dashes in generated pages. Guards the section prose and the
/// table placeholders, in both renderers.
#[test]
fn pages_contain_no_em_dashes() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    for format in ["md", "html"] {
        let dest = dir.path().join(format!("dash.{format}"));
        let page = run_summary(&rep, &out, "nrt_ar_ar", format, &dest);
        assert!(!page.contains('\u{2014}'), "{format} page contains an em dash\n{page}");
    }
}

/// A file with no extra parameters renders the word "none", not a dash.
#[test]
fn empty_extra_params_render_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let rep = dir.path().join("report");
    let out = dir.path().join("output");
    fs::create_dir_all(rep.join("header")).unwrap();
    fs::write(
        rep.join("header").join("bare.yaml.tsv"),
        "filename\thas_temp\thas_psal\thas_pres\thas_deph\thas_time_qc\thas_position_qc\textra_params\n\
         B1\ttrue\ttrue\ttrue\tfalse\ttrue\ttrue\t\n",
    )
    .unwrap();

    let md = run_summary(&rep, &out, "bare", "md", &dir.path().join("bare.md"));
    assert!(md.contains("| Extra parameters | none |"), "extras placeholder wrong\n{md}");
    assert!(!md.contains('\u{2014}'), "page contains an em dash\n{md}");
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

// ── Filter bounding box ───────────────────────────────────────────────────────

/// Platform-level TSV carrying the geographic extent columns the bounding-box
/// table aggregates. `rows` are (platform, lon_min, lon_max, lat_min, lat_max);
/// an empty cell (the report writer's rendering of NaN) is written as `""`.
fn write_bbox_tsv(path: &Path, rows: &[(&str, &str, &str, &str, &str)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut s = String::from(
        "platform_code\tn_profiles\tn_obs\tlongitude_min\tlongitude_max\tlatitude_min\tlatitude_max\n",
    );
    for (p, lo_min, lo_max, la_min, la_max) in rows {
        s.push_str(&format!("{p}\t10\t100\t{lo_min}\t{lo_max}\t{la_min}\t{la_max}\n"));
    }
    fs::write(path, s).unwrap();
}

#[test]
fn filter_bbox_table_reports_extremes_across_platforms() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = (dir.path().join("report"), dir.path().join("output"));
    // Extremes are the min of the mins and the max of the maxes over platforms.
    write_bbox_tsv(
        &rep.join("convert").join("bb.parquet.tsv"),
        &[("A", "-40.5", "12.25", "50.1", "80.9"), ("B", "3.5", "44.75", "35.0", "60.0")],
    );
    write_bbox_tsv(
        &rep.join("clean/filter").join("bb.parquet.tsv"),
        &[("A", "-5.6", "12.25", "51.0", "60.0"), ("B", "3.5", "30.0", "36.0", "45.5")],
    );

    let md = run_summary(&rep, &out, "bb", "md", &dir.path().join("bb.md"));
    assert!(md.contains("**Bounding box**"), "bbox table missing\n{md}");
    assert!(md.contains("| Metric | Filtered | Original |"), "bbox headers wrong\n{md}");
    for row in [
        "| Longitude min | -5.600 | -40.500 |",
        "| Longitude max | 30.000 | 44.750 |",
        "| Latitude min | 36.000 | 35.000 |",
        "| Latitude max | 60.000 | 80.900 |",
    ] {
        assert!(md.contains(row), "missing bbox row: {row}\n{md}");
    }
    // The table belongs to the filter stage only.
    assert_eq!(md.matches("**Bounding box**").count(), 1, "bbox table duplicated\n{md}");
    assert!(!md.contains('\u{2014}'), "page contains an em dash\n{md}");
}

#[test]
fn filter_bbox_omits_original_column_without_convert_report() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = (dir.path().join("report"), dir.path().join("output"));
    write_bbox_tsv(
        &rep.join("clean/filter").join("bb.parquet.tsv"),
        &[("A", "-5.6", "12.25", "51.0", "60.0")],
    );

    let md = run_summary(&rep, &out, "bb", "md", &dir.path().join("bb.md"));
    assert!(md.contains("| Metric | Filtered |"), "bbox headers wrong\n{md}");
    assert!(!md.contains("Original |"), "unexpected Original column\n{md}");
    assert!(md.contains("| Longitude min | -5.600 |"), "bbox row wrong\n{md}");
}

#[test]
fn filter_bbox_table_absent_when_no_valid_positions() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = (dir.path().join("report"), dir.path().join("output"));
    // Every extent cell empty, as the report writer renders an all-NaN position.
    write_bbox_tsv(&rep.join("clean/filter").join("bb.parquet.tsv"), &[("A", "", "", "", "")]);

    let md = run_summary(&rep, &out, "bb", "md", &dir.path().join("bb.md"));
    assert!(md.contains("### Filter by region"), "filter section missing\n{md}");
    assert!(!md.contains("Bounding box"), "bbox table should be omitted\n{md}");
}

#[test]
fn filter_bbox_renders_in_html() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = (dir.path().join("report"), dir.path().join("output"));
    write_bbox_tsv(
        &rep.join("convert").join("bb.parquet.tsv"),
        &[("A", "-40.5", "12.25", "50.1", "80.9")],
    );
    write_bbox_tsv(
        &rep.join("clean/filter").join("bb.parquet.tsv"),
        &[("A", "-5.6", "12.25", "51.0", "60.0")],
    );

    let html = run_summary(&rep, &out, "bb", "html", &dir.path().join("bb.html"));
    assert!(html.contains("Bounding box</p>"), "bbox title missing\n{html}");
    assert!(html.contains("<th>Filtered</th><th>Original</th>"), "bbox headers wrong\n{html}");
    assert!(
        html.contains("<td>Longitude min</td><td>-5.600</td><td>-40.500</td>"),
        "bbox row wrong\n{html}"
    );
}

// ── Comparison section ─────────────────────────────────────────────────────────

/// Write a `compare` TSV (the two-direction output of `ctddump compare`). Each row
/// is (reference, compared, ref_platforms, common_platforms, ref_profiles,
/// ref_unkeyed, matched_profiles, same_nobs, diff_nobs, ref_obs, matched_obs). The
/// three percentage columns are filled with a wrong sentinel (`999`) to prove the
/// summary recomputes them from the counts instead of trusting the file.
#[allow(clippy::type_complexity)]
fn write_compare_tsv(path: &Path, rows: &[(&str, &str, u64, u64, u64, u64, u64, u64, u64, u64, u64)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut s = String::from(
        "reference\tcompared\tref_platforms\tcommon_platforms\tplatform_cov_pct\t\
         ref_profiles\tref_unkeyed_profiles\tmatched_profiles\tprofile_cov_pct\t\
         same_nobs\tdiff_nobs\tnobs_agree_pct\tref_observations\tmatched_observations\n",
    );
    for (rf, cmp, rp, cp, rprof, unk, mprof, same, diff, robs, mobs) in rows {
        s.push_str(&format!(
            "{rf}\t{cmp}\t{rp}\t{cp}\t999\t{rprof}\t{unk}\t{mprof}\t999\t{same}\t{diff}\t999\t{robs}\t{mobs}\n"
        ));
    }
    fs::write(path, s).unwrap();
}

#[test]
fn comparison_section_gathers_rows_referencing_the_stem() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    // nrt_ar_ar is the reference in two compare files (vs CORA and vs GL). Each file
    // holds both directions; only the row whose reference is nrt_ar_ar is picked up.
    let cmp = rep.join("compare");
    write_compare_tsv(
        &cmp.join("ar_nrt_vs_cora.tsv"),
        &[
            ("nrt_ar_ar", "cora_ar", 2, 1, 200, 10, 150, 120, 30, 20000, 15000),
            // reverse direction (reference cora_ar) must not appear on this page.
            ("cora_ar", "nrt_ar_ar", 5, 1, 400, 0, 150, 120, 30, 40000, 15000),
        ],
    );
    write_compare_tsv(
        &cmp.join("ar_nrt_vs_gl.tsv"),
        &[
            ("nrt_ar_ar", "nrt_ar_gl", 2, 2, 200, 10, 200, 200, 0, 20000, 20000),
            ("nrt_ar_gl", "nrt_ar_ar", 3, 2, 210, 0, 200, 200, 0, 21000, 20000),
        ],
    );

    let md = run_summary(&rep, &out, "nrt_ar_ar", "md", &dir.path().join("c.md"));

    assert!(md.contains("## Comparison"), "no comparison section\n{md}");
    // Reference totals, taken from the first row (identical across rows). The
    // unkeyed row is unique to this section, so it pins the totals table.
    assert!(md.contains("| Profiles without a key | 10 |"), "unkeyed total wrong\n{md}");
    // Coverage rows are sorted by the compared name: cora_ar before nrt_ar_gl.
    // cora_ar: platform 1/2 = 50.0%, profile 150/200 = 75.0%, agreement 120/150 = 80.0%.
    assert!(
        md.contains("| cora_ar | 1 | 50.0% | 150 | 75.0% | 120 | 30 | 80.0% |"),
        "cora coverage row wrong\n{md}"
    );
    // nrt_ar_gl: platform 2/2, profile 200/200, agreement 200/200, all 100.0%.
    assert!(
        md.contains("| nrt_ar_gl | 2 | 100.0% | 200 | 100.0% | 200 | 0 | 100.0% |"),
        "gl coverage row wrong\n{md}"
    );
    // The reverse-direction rows (reference cora_ar / nrt_ar_gl) are not shown.
    assert!(!md.contains("| nrt_ar_ar |"), "reverse direction leaked in\n{md}");
    // Percentages are recomputed from the counts, not the bogus 999 in the TSV.
    assert!(!md.contains("999"), "trusted the TSV percentage column\n{md}");
    // Both compare files are listed as sources.
    assert!(md.contains("ar_nrt_vs_cora.tsv") && md.contains("ar_nrt_vs_gl.tsv"), "source files missing\n{md}");
    // House style.
    assert!(!md.contains('\u{2014}'), "comparison prose has an em dash\n{md}");

    // Same section renders in HTML.
    let html = run_summary(&rep, &out, "nrt_ar_ar", "html", &dir.path().join("c.html"));
    assert!(html.contains("<h2>Comparison</h2>"), "no html comparison heading\n{html}");
    assert!(
        html.contains("<td>cora_ar</td><td>1</td><td>50.0%</td><td>150</td><td>75.0%</td>"),
        "html coverage row wrong\n{html}"
    );
}

#[test]
fn comparison_section_absent_without_matching_reports() {
    let dir = tempfile::tempdir().unwrap();
    let (rep, out) = full_tree(dir.path(), "nrt_ar_ar");
    // A compare dir exists but only references other stems.
    write_compare_tsv(
        &rep.join("compare").join("bo_nrt_vs_cora.tsv"),
        &[("nrt_bo_bo", "cora_bo", 2, 1, 100, 0, 50, 40, 10, 5000, 2500)],
    );

    let md = run_summary(&rep, &out, "nrt_ar_ar", "md", &dir.path().join("c.md"));
    assert!(!md.contains("## Comparison"), "comparison should be absent\n{md}");
    // The rest of the page is unaffected.
    assert!(md.contains("## Conversion"), "conversion missing\n{md}");
}
