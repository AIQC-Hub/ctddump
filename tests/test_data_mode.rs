//! Where `temp_dm` / `psal_dm` / `pres_dm` come from when a file does not carry
//! a per-observation `<PARAM>_DM` variable: the `_dm` cells hold a code only
//! when the file says so, and are empty (unknown) otherwise. Built on small
//! NetCDF files written here so each case is explicit.

use std::path::Path;

use ctddump::convert::common::{dm_note_profile_level, dm_notes_time_depth};
use ctddump::convert::cora_config::CoraConfig;
use ctddump::convert::source::SourceSpec;
use ctddump::handle_dispatch;
use netcdf::types::NcVariableType;
use netcdf::NcTypeDescriptor;
use polars::prelude::*;

#[repr(transparent)]
#[derive(Copy, Clone)]
struct NcChar(i8);

unsafe impl NcTypeDescriptor for NcChar {
    fn type_descriptor() -> NcVariableType {
        NcVariableType::Char
    }
}

/// An NRT-style file (`TIME` x `DEPTH`) with TEMP, PSAL and PRES and no
/// `<PARAM>_DM` variables. Each parameter's `data_mode` attribute is set from
/// `modes` (`None`: no attribute).
fn write_nrt(path: &Path, modes: [Option<&str>; 3]) {
    let mut f = netcdf::create(path).unwrap();
    f.add_dimension("TIME", 2).unwrap();
    f.add_dimension("DEPTH", 2).unwrap();
    f.add_attribute("platform_code", "TESTPLAT").unwrap();

    let mut t = f.add_variable::<f64>("TIME", &["TIME"]).unwrap();
    t.put_values(&[20_000.0, 20_001.0], ..).unwrap();
    let mut lat = f.add_variable::<f32>("LATITUDE", &["TIME"]).unwrap();
    lat.put_values(&[60.0f32, 60.5], ..).unwrap();
    let mut lon = f.add_variable::<f32>("LONGITUDE", &["TIME"]).unwrap();
    lon.put_values(&[5.0f32, 5.5], ..).unwrap();

    for (name, mode, values) in [
        ("TEMP", modes[0], [10.0f32, 9.0, 11.0, 8.0]),
        ("PSAL", modes[1], [35.0, 35.1, 35.2, 35.3]),
        ("PRES", modes[2], [1.0, 2.0, 1.0, 2.0]),
    ] {
        let mut v = f.add_variable::<f32>(name, &["TIME", "DEPTH"]).unwrap();
        v.set_fill_value(99_999.0f32).unwrap();
        if let Some(m) = mode {
            v.put_attribute("data_mode", m).unwrap();
        }
        v.put_values(&values, ..).unwrap();
        let mut qc = f.add_variable::<i8>(&format!("{name}_QC"), &["TIME", "DEPTH"]).unwrap();
        qc.put_values(&[1i8, 1, 1, 1], ..).unwrap();
    }
}

/// A CORA-style file (`N_PROF` x `N_LEVELS`), optionally with a per-profile
/// `DATA_MODE` holding `data_mode` (one char per profile).
fn write_cora(path: &Path, data_mode: Option<&str>) {
    let mut f = netcdf::create(path).unwrap();
    f.add_dimension("N_PROF", 2).unwrap();
    f.add_dimension("N_LEVELS", 2).unwrap();
    f.add_dimension("STRING8", 8).unwrap();

    let mut plat = f.add_variable_with_type("PLATFORM_NUMBER", &["N_PROF", "STRING8"], &NcVariableType::Char).unwrap();
    let mut names: Vec<NcChar> = Vec::new();
    for _ in 0..2 {
        let mut n: Vec<NcChar> = "6900001".bytes().map(|b| NcChar(b as i8)).collect();
        n.resize(8, NcChar(0));
        names.extend(n);
    }
    plat.put_values(&names, ..).unwrap();

    if let Some(dm) = data_mode {
        let mut v = f.add_variable_with_type("DATA_MODE", &["N_PROF"], &NcVariableType::Char).unwrap();
        let cells: Vec<NcChar> = dm.bytes().map(|b| NcChar(b as i8)).collect();
        v.put_values(&cells, ..).unwrap();
    }

    let mut t = f.add_variable::<f64>("TIME", &["N_PROF"]).unwrap();
    t.put_values(&[20_000.0, 20_001.0], ..).unwrap();
    let mut lat = f.add_variable::<f64>("LATITUDE", &["N_PROF"]).unwrap();
    lat.put_values(&[60.0, 60.5], ..).unwrap();
    let mut lon = f.add_variable::<f64>("LONGITUDE", &["N_PROF"]).unwrap();
    lon.put_values(&[5.0, 5.5], ..).unwrap();

    for (name, values) in [("TEMP", [10.0f32, 9.0, 11.0, 8.0]), ("PSAL", [35.0, 35.1, 35.2, 35.3]), ("PRES", [1.0, 2.0, 1.0, 2.0])] {
        let mut v = f.add_variable::<f32>(name, &["N_PROF", "N_LEVELS"]).unwrap();
        v.set_fill_value(99_999.0f32).unwrap();
        v.put_values(&values, ..).unwrap();
        let mut qc = f.add_variable::<i8>(&format!("{name}_QC"), &["N_PROF", "N_LEVELS"]).unwrap();
        qc.put_values(&[1i8, 1, 1, 1], ..).unwrap();
    }
}

fn convert(target: &str, src: &Path, dest: &Path) -> DataFrame {
    let args: Vec<String> = ["convert", target, src.to_str().unwrap(), dest.to_str().unwrap()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    handle_dispatch(&args).expect("conversion should succeed");
    ParquetReader::new(std::fs::File::open(dest).unwrap()).finish().unwrap()
}

fn strs(df: &DataFrame, col: &str) -> Vec<String> {
    df.column(col).unwrap().str().unwrap().into_iter().map(|s| s.unwrap_or("<null>").to_string()).collect()
}

/// A mixed (`"M"`) attribute without a `<PARAM>_DM` variable says the mode
/// varies per observation but not how: unknown, never `R`. A missing attribute
/// is unknown too. A plain `R`/`A`/`D` attribute is used as before.
#[test]
fn nrt_mixed_or_missing_attribute_without_dm_variable_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("AR_PR_CT_TEST.nc");
    write_nrt(&src, [Some("M"), None, Some("D")]);

    let df = convert("nrt_ar", &src, &dir.path().join("out.parquet"));
    assert_eq!(df.height(), 4);
    assert!(strs(&df, "temp_dm").iter().all(|d| d.is_empty()), "{:?}", strs(&df, "temp_dm"));
    assert!(strs(&df, "psal_dm").iter().all(|d| d.is_empty()), "{:?}", strs(&df, "psal_dm"));
    assert!(strs(&df, "pres_dm").iter().all(|d| d == "D"), "{:?}", strs(&df, "pres_dm"));
}

/// The notes behind the per-file warning name each parameter whose data mode
/// is unknown and why; parameters with a usable attribute are not mentioned.
#[test]
fn nrt_dm_notes_name_the_unknown_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("nrt.nc");
    write_nrt(&src, [Some("M"), None, Some("R")]);
    let file = netcdf::open(&src).unwrap();
    let spec = SourceSpec::nrt();

    let notes = dm_notes_time_depth(&file, &spec, &[&spec.temp, &spec.psal, &spec.pres, &spec.deph]);
    assert_eq!(
        notes,
        [
            "TEMP has data_mode \"M\" but no TEMP_DM variable",
            "PSAL has no PSAL_DM variable and no data_mode attribute",
        ]
    );

    let clean = dir.path().join("clean.nc");
    write_nrt(&clean, [Some("R"), Some("D"), Some("A")]);
    let file = netcdf::open(&clean).unwrap();
    assert!(dm_notes_time_depth(&file, &spec, &[&spec.temp, &spec.psal, &spec.pres]).is_empty());
}

/// CORA's `DATA_MODE` is read per profile; a fill cell is unknown for that
/// profile only.
#[test]
fn cora_data_mode_is_read_per_profile() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("cora.nc");
    write_cora(&src, Some("D "));

    let df = convert("cora", &src, &dir.path().join("out.parquet"));
    for col in ["temp_dm", "psal_dm", "pres_dm"] {
        assert_eq!(strs(&df, col), ["D", "D", "", ""], "{col}");
    }
    let file = netcdf::open(&src).unwrap();
    assert_eq!(dm_note_profile_level(&file, &SourceSpec::cora(&CoraConfig::cora())), None);
}

/// A CORA file without `DATA_MODE` has no data mode at all: every cell is
/// unknown (previously `R`), and the warning note says why.
#[test]
fn cora_without_data_mode_variable_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("cora.nc");
    write_cora(&src, None);

    let df = convert("cora", &src, &dir.path().join("out.parquet"));
    assert_eq!(df.height(), 4);
    for col in ["temp_dm", "psal_dm", "pres_dm"] {
        assert!(strs(&df, col).iter().all(|d| d.is_empty()), "{col}: {:?}", strs(&df, col));
    }
    let file = netcdf::open(&src).unwrap();
    assert_eq!(
        dm_note_profile_level(&file, &SourceSpec::cora(&CoraConfig::cora())).as_deref(),
        Some("no DATA_MODE variable")
    );
}
