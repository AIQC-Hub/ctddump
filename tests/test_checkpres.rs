//! `checkpres`: profiles whose pressure decreases in observation order.

use std::collections::HashMap;
use std::path::Path;

use ctddump::handle_dispatch;
use polars::prelude::*;

/// One observation row: (platform, profile_no, pres, pres_qc).
type Row<'a> = (&'a str, u32, f32, &'a str);

/// Write a per-file Parquet like `convert` does: rows in source order, with
/// `observation_no` counting from 1 within each profile.
fn write_file(path: &Path, rows: &[Row]) {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let mut obs = Vec::new();
    let mut counter: HashMap<(&str, u32), u32> = HashMap::new();
    for (p, n, _, _) in rows {
        let c = counter.entry((p, *n)).or_insert(0);
        *c += 1;
        obs.push(*c);
    }
    let mut df = df!(
        "platform_code"     => rows.iter().map(|r| r.0).collect::<Vec<_>>(),
        "profile_no"        => rows.iter().map(|r| r.1).collect::<Vec<_>>(),
        "profile_time"      => rows.iter().map(|r| 20_000.0 + r.1 as f64).collect::<Vec<_>>(),
        "profile_timestamp" => rows.iter().map(|r| 1_000_000_000_000i64 + r.1 as i64 * 86_400_000).collect::<Vec<_>>(),
        "observation_no"    => obs,
        "longitude"         => rows.iter().map(|_| 10.5f64).collect::<Vec<_>>(),
        "latitude"          => rows.iter().map(|_| 60.25f64).collect::<Vec<_>>(),
        "filename"          => rows.iter().map(|_| stem).collect::<Vec<_>>(),
        "pres"              => rows.iter().map(|r| r.2).collect::<Vec<_>>(),
        "pres_qc"           => rows.iter().map(|r| r.3).collect::<Vec<_>>(),
        "pres_conv"         => rows.iter().map(|_| 0i8).collect::<Vec<_>>(),
    )
    .unwrap();
    ParquetWriter::new(std::fs::File::create(path).unwrap()).finish(&mut df).unwrap();
}

fn run(extra: &[&str], src: &Path, prefix: &Path) {
    let mut args: Vec<String> = vec!["checkpres".into()];
    args.extend(extra.iter().map(|s| s.to_string()));
    args.push(src.to_str().unwrap().into());
    args.push(prefix.to_str().unwrap().into());
    handle_dispatch(&args).expect("checkpres should succeed");
}

/// Read a TSV into one map per data row, keyed by the header.
fn tsv(path: &Path) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    lines
        .map(|l| header.iter().map(|h| h.to_string()).zip(l.split('\t').map(String::from)).collect())
        .collect()
}

fn out(prefix: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{}", prefix.display(), suffix))
}

/// One file with two platforms and every kind of profile.
fn mixed_rows() -> Vec<Row<'static>> {
    let mut rows = Vec::new();
    let mut add = |p: &'static str, n: u32, pres: &[f32]| {
        rows.extend(pres.iter().map(|&v| (p, n, v, "1")));
    };
    add("A", 1, &[1.0, 2.0, 3.0]); // increasing
    add("A", 2, &[1.0, 2.0, 2.0, 3.0]); // repeated pressure: fine
    add("A", 3, &[30.0, 20.0, 20.0, 10.0]); // reversed
    add("B", 4, &[1.0, 5.0, 3.0, 7.0, 6.0]); // non-monotonic, two decreases
    add("B", 5, &[1.0, f32::NAN, 2.0, f32::NAN, 0.5]); // a decrease across missing pressures
    add("B", 6, &[5.0]); // a single pressure: nothing to compare
    add("C", 7, &[f32::NAN, f32::NAN]); // no pressure: not checked, not counted
    rows
}

#[test]
fn flags_decreases_and_classifies_profiles() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("f1.parquet"), &mixed_rows());
    let prefix = dir.path().join("out/unit");
    run(&[], &dir.path().join("f1.parquet"), &prefix);

    let prof = tsv(&out(&prefix, ".profiles.tsv"));
    let got: Vec<(&str, &str, &str, &str, &str, &str)> = prof
        .iter()
        .map(|r| {
            (
                r["platform_code"].as_str(),
                r["profile_no"].as_str(),
                r["kind"].as_str(),
                r["n_decreases"].as_str(),
                r["first_decrease_obs"].as_str(),
                r["max_decrease"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("A", "3", "reversed", "2", "2", "10.000"),
            ("B", "4", "non_monotonic", "2", "3", "2.000"),
            ("B", "5", "non_monotonic", "1", "5", "1.500"),
        ]
    );
    let p5 = &prof[2];
    assert_eq!((p5["n_obs"].as_str(), p5["n_pres"].as_str()), ("5", "3"));
    assert_eq!(
        (p5["pres_first"].as_str(), p5["pres_last"].as_str(), p5["pres_min"].as_str(), p5["pres_max"].as_str()),
        ("1", "0.5", "0.5", "2")
    );
    assert_eq!(p5["filename"], "f1");
    assert_eq!(p5["profile_timestamp"], "2001-09-14 01:46:40");
    assert_eq!((p5["longitude"].as_str(), p5["latitude"].as_str()), ("10.5", "60.25"));

    let plat = tsv(&out(&prefix, ".platforms.tsv"));
    assert_eq!(plat.len(), 2);
    assert_eq!(
        (plat[0]["platform_code"].as_str(), plat[0]["n_profiles"].as_str(), plat[0]["n_bad_profiles"].as_str()),
        ("A", "3", "1")
    );
    assert_eq!((plat[0]["pct_bad"].as_str(), plat[0]["n_reversed"].as_str()), ("33.33", "1"));
    assert_eq!(
        (plat[1]["platform_code"].as_str(), plat[1]["n_bad_profiles"].as_str(), plat[1]["n_non_monotonic"].as_str()),
        ("B", "2", "2")
    );
    assert_eq!((plat[1]["first_date"].as_str(), plat[1]["last_date"].as_str()), ("2001-09-13", "2001-09-14"));

    let sum = tsv(&out(&prefix, ".summary.tsv"));
    assert_eq!(sum.len(), 1);
    let s = &sum[0];
    for (k, v) in [
        ("n_files", "1"),
        ("n_platforms", "2"),
        ("n_profiles", "6"),
        ("n_no_pres", "1"),
        ("n_bad_profiles", "3"),
        ("pct_bad", "50.00"),
        ("n_reversed", "1"),
        ("n_non_monotonic", "2"),
        ("n_bad_platforms", "2"),
        ("n_bad_files", "1"),
    ] {
        assert_eq!(s[k], v, "{k}");
    }
}

/// `--min-drop` ignores small decreases; `--pres-qc` skips pressures with other flags.
#[test]
fn min_drop_and_pres_qc_options() {
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<Row> = vec![
        ("A", 1, 10.0, "1"),
        ("A", 1, 9.95, "1"),
        ("A", 1, 11.0, "1"),
        ("A", 2, 1.0, "1"),
        ("A", 2, 50.0, "4"),
        ("A", 2, 2.0, "1"),
    ];
    write_file(&dir.path().join("f.parquet"), &rows);
    let prefix = dir.path().join("o");
    let flagged = |extra: &[&str]| -> Vec<String> {
        run(extra, dir.path(), &prefix);
        tsv(&out(&prefix, ".profiles.tsv")).iter().map(|r| r["profile_no"].clone()).collect()
    };

    assert_eq!(flagged(&[]), ["1", "2"]);
    assert_eq!(flagged(&["--min-drop", "0.1"]), ["2"]);
    assert_eq!(flagged(&["--pres-qc", "1,2"]), ["1"]);
    assert_eq!(flagged(&["--min-drop", "0.1", "--pres-qc", "1"]), Vec::<String>::new());
}

/// A directory is checked file by file under `--pattern`; a platform found in
/// two files counts both, and the result does not depend on the thread count.
#[test]
fn directory_pattern_and_threads() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("a.parquet"), &[("A", 1, 2.0, "1"), ("A", 1, 1.0, "1")]);
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    write_file(&sub.join("b.parquet"), &[("A", 1, 1.0, "1"), ("A", 1, 2.0, "1"), ("B", 2, 3.0, "1"), ("B", 2, 1.0, "1")]);
    write_file(&dir.path().join("skip.parquet"), &[("C", 1, 2.0, "1"), ("C", 1, 1.0, "1")]);

    let read_all = |prefix: &Path| -> Vec<String> {
        [".profiles.tsv", ".platforms.tsv", ".summary.tsv"]
            .iter()
            .map(|s| std::fs::read_to_string(out(prefix, s)).unwrap())
            .collect()
    };
    let p_par = dir.path().join("par");
    let p_seq = dir.path().join("seq");
    run(&["--pattern", "[ab].parquet"], dir.path(), &p_par);
    run(&["--pattern", "[ab].parquet", "--threads", "1"], dir.path(), &p_seq);
    assert_eq!(read_all(&p_par), read_all(&p_seq));

    // Profile 1 of A appears in both files; only a.parquet's copy decreases.
    let prof = tsv(&out(&p_par, ".profiles.tsv"));
    let ids: Vec<(&str, &str)> = prof.iter().map(|r| (r["filename"].as_str(), r["platform_code"].as_str())).collect();
    assert_eq!(ids, [("a", "A"), ("b", "B")]);

    let plat = tsv(&out(&p_par, ".platforms.tsv"));
    assert_eq!((plat[0]["platform_code"].as_str(), plat[0]["n_files"].as_str(), plat[0]["n_profiles"].as_str()), ("A", "2", "2"));
    let s = &tsv(&out(&p_par, ".summary.tsv"))[0];
    assert_eq!((s["n_files"].as_str(), s["n_profiles"].as_str(), s["n_bad_files"].as_str()), ("2", "3", "2"));
}

/// No matching file: nothing is written and the command still succeeds.
#[test]
fn no_matching_files_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("o");
    run(&[], dir.path(), &prefix);
    assert!(!out(&prefix, ".summary.tsv").exists());
    // An absent directory is the same no-op, as for `concat`.
    run(&[], &dir.path().join("absent"), &prefix);
    assert!(!out(&prefix, ".summary.tsv").exists());
}

#[test]
fn missing_pres_column_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut df = df!("platform_code" => ["A"], "profile_no" => [1u32]).unwrap();
    ParquetWriter::new(std::fs::File::create(dir.path().join("x.parquet")).unwrap()).finish(&mut df).unwrap();
    let args: Vec<String> =
        ["checkpres", dir.path().to_str().unwrap(), dir.path().join("o").to_str().unwrap()].iter().map(|s| s.to_string()).collect();
    let err = handle_dispatch(&args).unwrap_err().to_string();
    assert!(err.contains("missing column `pres`"), "{err}");
}

#[test]
fn negative_min_drop_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let args: Vec<String> = ["checkpres", "--min-drop=-1", dir.path().to_str().unwrap(), dir.path().join("o").to_str().unwrap()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert!(handle_dispatch(&args).is_err());
}

/// A real CORA file whose levels are stored out of order: five profiles on two
/// platforms, all non-monotonic.
#[test]
fn cora_fixture_with_shuffled_levels() {
    let dir = tempfile::tempdir().unwrap();
    let pq = dir.path().join("CO_DMQCGL01_19861204_PR_CT.parquet");
    let args: Vec<String> = ["convert", "cora", "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc", pq.to_str().unwrap()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    handle_dispatch(&args).unwrap();
    let prefix = dir.path().join("o");
    run(&[], dir.path(), &prefix);

    let s = &tsv(&out(&prefix, ".summary.tsv"))[0];
    assert_eq!(
        (s["n_bad_profiles"].as_str(), s["n_non_monotonic"].as_str(), s["n_bad_platforms"].as_str()),
        ("5", "5", "2")
    );
    let prof = tsv(&out(&prefix, ".profiles.tsv"));
    let y7bo3 = prof.iter().find(|r| r["platform_code"] == "Y7BO" && r["profile_no"] == "3").unwrap();
    assert_eq!((y7bo3["n_pres"].as_str(), y7bo3["n_decreases"].as_str()), ("17", "10"));
}

/// Many small files, as in a CORA directory. A rayon pool ran each file's check
/// as a task, and a worker waiting inside Polars stole and nested other files'
/// checks on its own stack until it overflowed (seen at 20,000 CORA files).
/// The plain worker threads never nest, so this completes.
#[test]
fn many_small_files_do_not_overflow_the_stack() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir(&src).unwrap();
    write_file(&src.join("f00000.parquet"), &[("A", 1, 2.0, "1"), ("A", 1, 1.0, "1")]);
    let n = 3000;
    for i in 1..n {
        std::fs::copy(src.join("f00000.parquet"), src.join(format!("f{i:05}.parquet"))).unwrap();
    }
    let prefix = dir.path().join("o");
    run(&["--threads", "4"], &src, &prefix);
    let s = &tsv(&out(&prefix, ".summary.tsv"))[0];
    assert_eq!((s["n_files"].as_str(), s["n_profiles"].as_str()), ("3000", "3000"));
}
