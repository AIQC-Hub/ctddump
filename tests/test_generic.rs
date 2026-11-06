//! `convert generic` / `batch convert generic` / `header generic` on NetCDF files
//! built here, so they cover non-Copernicus names, units and storage types
//! without needing extra fixtures.

use std::fs::File;
use std::path::{Path, PathBuf};

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

fn chars(s: &str, len: usize) -> Vec<NcChar> {
    let mut v: Vec<NcChar> = s.bytes().map(|b| NcChar(b as i8)).collect();
    v.resize(len, NcChar(0));
    v
}

const FILL_F32: f32 = -999.0;

/// A mooring-style time-depth file with non-Copernicus names: `time` x `z`,
/// seconds since 1970, a scalar latitude and no longitude, `i16` QC, packed
/// salinity, pressure only, no platform attribute.
fn write_mooring(path: &Path, time_units: Option<&str>, calendar: Option<&str>) {
    let mut f = netcdf::create(path).unwrap();
    f.add_dimension("time", 3).unwrap();
    f.add_dimension("z", 4).unwrap();
    f.add_attribute("title", "Test mooring").unwrap();
    f.add_attribute("station_id", 42i32).unwrap();

    let mut t = f.add_variable::<f64>("time", &["time"]).unwrap();
    if let Some(u) = time_units {
        t.put_attribute("units", u).unwrap();
    }
    if let Some(c) = calendar {
        t.put_attribute("calendar", c).unwrap();
    }
    t.put_values(&[0.0, 3_600.0, 86_400.0], ..).unwrap();

    let mut lat = f.add_variable::<f32>("lat", &[]).unwrap();
    lat.put_values(&[60.0f32], ..).unwrap();

    let mut temp = f.add_variable::<f32>("temperature", &["time", "z"]).unwrap();
    temp.set_fill_value(FILL_F32).unwrap();
    #[rustfmt::skip]
    temp.put_values(&[
        10.0, 9.0, 8.0, 7.0,
        11.0, FILL_F32, 9.0, 8.0,
        FILL_F32, FILL_F32, FILL_F32, FILL_F32,
    ], ..).unwrap();

    let mut qc = f.add_variable::<i16>("temperature_flag", &["time", "z"]).unwrap();
    qc.set_fill_value(-32_767i16).unwrap();
    #[rustfmt::skip]
    qc.put_values(&[
        1i16, 1, 1, 1,
        1, -32_767, 4, 1,
        -32_767, -32_767, -32_767, -32_767,
    ], ..).unwrap();

    // Packed: value = raw * 0.01 + 30.
    let mut sal = f.add_variable::<i16>("sal", &["time", "z"]).unwrap();
    sal.set_fill_value(-1i16).unwrap();
    sal.put_attribute("scale_factor", 0.01f64).unwrap();
    sal.put_attribute("add_offset", 30.0f64).unwrap();
    sal.put_values(&[100i16, 120, 140, 160, 100, 120, 140, 160, -1, -1, -1, -1], ..).unwrap();

    let mut pres = f.add_variable::<f64>("pressure", &["time", "z"]).unwrap();
    pres.put_values(&[1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0], ..).unwrap();
}

const MOORING_MAP: &str = r#"
[dimensions]
profile = "time"
level = "z"

[variables]
time = "time"
latitude = "lat"
longitude = "lon"
temp = "temperature"
temp_qc = "temperature_flag"
psal = "sal"
pres = "pressure"

[defaults]
platform_code = "MOOR1"
longitude = 20.0
data_mode = "D"
"#;

/// A cruise-style profile-level file: `cast` x `level`, a char platform variable
/// (second cast blank), days since 2000, depth only, char QC, no data mode.
fn write_cruise(path: &Path) {
    let mut f = netcdf::create(path).unwrap();
    f.add_dimension("cast", 2).unwrap();
    f.add_dimension("level", 3).unwrap();
    f.add_dimension("strlen", 8).unwrap();

    let mut ship = f.add_variable_with_type("ship", &["cast", "strlen"], &NcVariableType::Char).unwrap();
    let mut names = chars("SHIP_A", 8);
    names.extend(chars("", 8));
    ship.put_values(&names, ..).unwrap();

    let mut t = f.add_variable::<f64>("time", &["cast"]).unwrap();
    t.put_attribute("units", "days since 2000-01-01T00:00:00Z").unwrap();
    t.put_values(&[0.5, 1.0], ..).unwrap();

    let mut lat = f.add_variable::<f64>("lat", &["cast"]).unwrap();
    lat.put_values(&[60.0, 61.0], ..).unwrap();
    let mut lon = f.add_variable::<f64>("lon", &["cast"]).unwrap();
    lon.put_values(&[20.0, 21.0], ..).unwrap();

    let mut depth = f.add_variable::<f32>("depth", &["cast", "level"]).unwrap();
    depth.put_values(&[5.0f32, 10.0, 15.0, 5.0, 10.0, 15.0], ..).unwrap();

    let mut temp = f.add_variable::<f32>("TEMP", &["cast", "level"]).unwrap();
    temp.put_values(&[5.0f32, 4.5, 4.0, 6.0, 5.5, 5.0], ..).unwrap();

    let mut qc = f.add_variable_with_type("TEMP_QC", &["cast", "level"], &NcVariableType::Char).unwrap();
    let flags: Vec<NcChar> = "141 11".bytes().map(|b| NcChar(b as i8)).collect();
    qc.put_values(&flags, ..).unwrap();
}

const CRUISE_MAP: &str = r#"
layout = "profile-level"

[dimensions]
profile = "cast"
level = "level"

[variables]
time = "time"
longitude = "lon"
latitude = "lat"
deph = "depth"

[platform]
variable = "ship"

[defaults]
platform_code = "SHIPX"
"#;

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn write_map(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn convert(map: Option<&Path>, extra: &[&str], src: &Path, dest: &Path) -> Result<(), String> {
    let mut args = vec!["convert".to_string(), "generic".to_string()];
    if let Some(m) = map {
        args.push("--config".to_string());
        args.push(s(m));
    }
    args.extend(extra.iter().map(|a| a.to_string()));
    args.push(s(src));
    args.push(s(dest));
    handle_dispatch(&args).map(|_| ()).map_err(|e| e.to_string())
}

fn read(path: &Path) -> DataFrame {
    ParquetReader::new(File::open(path).unwrap()).finish().unwrap()
}

fn strs(df: &DataFrame, col: &str) -> Vec<String> {
    df.column(col).unwrap().str().unwrap().into_iter().map(|v| v.unwrap_or("").to_string()).collect()
}

fn f64s(df: &DataFrame, col: &str) -> Vec<f64> {
    let c = df.column(col).unwrap().cast(&DataType::Float64).unwrap();
    c.f64().unwrap().into_iter().map(|v| v.unwrap_or(f64::NAN)).collect()
}

/// Unix milliseconds of 1950-01-01: the `profile_time` epoch.
const EPOCH_1950_MS: i64 = -631_152_000_000;

#[test]
fn time_depth_mapping_converts() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mooring.nc");
    let dest = dir.path().join("mooring.parquet");
    write_mooring(&src, Some("seconds since 1970-01-01 00:00:00"), None);
    let map = write_map(dir.path(), "map.toml", MOORING_MAP);

    convert(Some(&map), &[], &src, &dest).unwrap();
    let df = read(&dest);

    // 3 profiles x 4 levels; every row has a pressure, so none is dropped.
    assert_eq!(df.height(), 12);
    assert!(strs(&df, "platform_code").iter().all(|p| p == "MOOR1"));
    assert!(strs(&df, "filename").iter().all(|p| p == "mooring"));

    // Seconds since 1970 -> days since 1950 (1970-01-01 is day 7305).
    let t = f64s(&df, "profile_time");
    assert_eq!(t[0], 7305.0);
    assert!((t[4] - (7305.0 + 1.0 / 24.0)).abs() < 1e-9);
    assert_eq!(t[8], 7306.0);
    let ts = df.column("profile_timestamp").unwrap().cast(&DataType::Int64).unwrap();
    let ts: Vec<i64> = ts.i64().unwrap().into_no_null_iter().collect();
    assert_eq!(ts[0], 0, "1970-01-01T00:00:00Z");
    assert_eq!(ts[8], 86_400_000);
    assert_eq!(ts[0] - EPOCH_1950_MS, 7305 * 86_400_000);

    // Scalar latitude is broadcast; the missing longitude takes the default.
    assert!(f64s(&df, "latitude").iter().all(|&v| v == 60.0));
    assert!(f64s(&df, "longitude").iter().all(|&v| v == 20.0));

    // _FillValue -> NaN; i16 QC with -32767 fill; packed salinity unpacked.
    let temp = f64s(&df, "temp");
    assert_eq!(temp[0], 10.0);
    assert!(temp[5].is_nan());
    assert!(temp[8..].iter().all(|v| v.is_nan()));
    let qc = strs(&df, "temp_qc");
    assert_eq!(&qc[..8], &["1", "1", "1", "1", "1", "", "4", "1"]);
    let psal = f64s(&df, "psal");
    assert!((psal[0] - 31.0).abs() < 1e-4);
    assert!((psal[3] - 31.6).abs() < 1e-4);
    assert!(psal[8].is_nan());

    // Depth derived from pressure; data mode from the default.
    let deph = f64s(&df, "deph");
    assert!(deph[0] > 0.9 && deph[0] < 1.1, "1 dbar is about 1 m, got {}", deph[0]);
    assert!(f64s(&df, "deph_conv").iter().all(|&c| c == 1.0));
    assert!(strs(&df, "temp_dm").iter().all(|d| d == "D"));
    assert!(strs(&df, "pres_dm").iter().all(|d| d == "D"));
    // No QC variable for pressure, none for time/position.
    assert!(strs(&df, "pres_qc").iter().all(String::is_empty));
    assert!(strs(&df, "time_qc").iter().all(String::is_empty));
}

#[test]
fn profile_level_mapping_converts() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("cruise.nc");
    let dest = dir.path().join("cruise.parquet");
    write_cruise(&src);
    let map = write_map(dir.path(), "map.toml", CRUISE_MAP);

    convert(Some(&map), &[], &src, &dest).unwrap();
    let df = read(&dest);

    assert_eq!(df.height(), 6);
    // Blank platform entry takes the default.
    assert_eq!(strs(&df, "platform_code"), ["SHIP_A", "SHIP_A", "SHIP_A", "SHIPX", "SHIPX", "SHIPX"]);
    // 2000-01-01 is day 18262 after 1950-01-01.
    let t = f64s(&df, "profile_time");
    assert_eq!(t[0], 18262.5);
    assert_eq!(t[3], 18263.0);
    assert_eq!(f64s(&df, "longitude")[3], 21.0);
    // Pressure derived from depth, flagged as converted.
    let pres = f64s(&df, "pres");
    assert!(pres[0] > 5.0 && pres[0] < 5.2, "5 m is about 5.03 dbar, got {}", pres[0]);
    assert!(f64s(&df, "pres_conv").iter().all(|&c| c == 1.0));
    assert_eq!(f64s(&df, "deph")[2], 15.0);
    // Char QC detected; blank flag is empty.
    assert_eq!(strs(&df, "temp_qc"), ["1", "4", "1", "", "1", "1"]);
    // No salinity variable at all.
    assert!(f64s(&df, "psal").iter().all(|v| v.is_nan()));
    // No DATA_MODE variable and no default: unknown, not assumed real-time.
    assert!(strs(&df, "temp_dm").iter().all(|d| d.is_empty()));
}

#[test]
fn cli_flags_override_the_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mooring.nc");
    let dest = dir.path().join("mooring.parquet");
    // No units attribute: --time-units supplies them.
    write_mooring(&src, None, None);
    let map = write_map(dir.path(), "map.toml", MOORING_MAP);

    convert(
        Some(&map),
        &["--time-units", "hours since 1950-01-01", "--platform-code", "OVERRIDE"],
        &src,
        &dest,
    )
    .unwrap();
    let df = read(&dest);
    assert!(strs(&df, "platform_code").iter().all(|p| p == "OVERRIDE"));
    assert_eq!(f64s(&df, "profile_time")[4], 3_600.0 / 24.0);
}

#[test]
fn generic_without_mapping_matches_nrt_ar() {
    // An empty mapping describes a Copernicus NRT file; for an Arctic file
    // (no profile coordinates) the output must equal `convert nrt_ar`.
    let dir = tempfile::tempdir().unwrap();
    let src = Path::new("./tests/test_data/AR_PR_CT_ITP-71.nc");
    let nrt = dir.path().join("nrt.parquet");
    let generic = dir.path().join("generic.parquet");

    handle_dispatch(&["convert".into(), "nrt_ar".into(), s(src), s(&nrt)]).unwrap();
    convert(None, &[], src, &generic).unwrap();

    // Same writer, same data: the files must be byte-identical.
    assert!(std::fs::read(&nrt).unwrap() == std::fs::read(&generic).unwrap(), "generic output differs from nrt_ar");
}

fn expect_error(map_body: &str, write: impl Fn(&Path), needle: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.nc");
    write(&src);
    let map = write_map(dir.path(), "map.toml", map_body);
    let err = convert(Some(&map), &[], &src, &dir.path().join("out.parquet")).unwrap_err();
    assert!(err.contains(needle), "expected \"{needle}\" in: {err}");
}

#[test]
fn missing_time_units_is_an_error() {
    expect_error(MOORING_MAP, |p| write_mooring(p, None, None), "has no units attribute");
}

#[test]
fn unsupported_calendar_is_an_error() {
    expect_error(MOORING_MAP, |p| write_mooring(p, Some("days since 2000-01-01"), Some("noleap")), "calendar \"noleap\"");
}

#[test]
fn missing_dimension_is_an_error() {
    let map = MOORING_MAP.replace("level = \"z\"", "level = \"depth\"");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "dimension \"depth\" not found");
}

#[test]
fn missing_measurements_are_an_error() {
    let map = MOORING_MAP.replace("temp = \"temperature\"", "temp = \"T\"").replace("psal = \"sal\"", "psal = \"S\"");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "neither temperature");
}

#[test]
fn missing_platform_without_default_is_an_error() {
    let map = MOORING_MAP.replace("platform_code = \"MOOR1\"", "");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "platform global attribute \"platform_code\" not found");
}

#[test]
fn missing_position_without_default_is_an_error() {
    let map = MOORING_MAP.replace("longitude = 20.0", "");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "longitude variable \"lon\" not found");
}

#[test]
fn wrong_shape_is_an_error() {
    // Latitude mapped to a [time, z] variable.
    let map = MOORING_MAP.replace("latitude = \"lat\"", "latitude = \"pressure\"");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "variable \"pressure\" has dimensions");
}

#[test]
fn both_platform_sources_are_an_error() {
    let map = format!("{CRUISE_MAP}\n").replace("variable = \"ship\"", "variable = \"ship\"\nattribute = \"title\"");
    expect_error(&map, write_cruise, "either attribute or variable");
}

#[test]
fn unknown_mapping_key_is_an_error() {
    let map = MOORING_MAP.replace("temp = \"temperature\"", "temperature = \"temperature\"");
    expect_error(&map, |p| write_mooring(p, Some("days since 1950-01-01"), None), "unknown field");
}

#[test]
fn batch_convert_generic() {
    let src_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    write_cruise(&src_dir.path().join("cast_a.nc"));
    write_cruise(&src_dir.path().join("cast_b.nc"));
    write_cruise(&src_dir.path().join("other.nc"));
    let map = write_map(src_dir.path(), "map.toml", &format!("pattern = \"cast_*.nc\"\n{CRUISE_MAP}"));

    handle_dispatch(&[
        "batch".into(), "convert".into(), "generic".into(),
        "--config".into(), s(&map),
        "-o".into(), s(out_dir.path()),
        s(src_dir.path()),
    ])
    .unwrap();

    let mut out: Vec<String> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    assert_eq!(out, ["cast_a.parquet", "cast_b.parquet"]);
    assert_eq!(read(&out_dir.path().join("cast_a.parquet")).height(), 6);
}

#[test]
fn header_generic_keeps_all_global_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mooring.nc");
    let dest = dir.path().join("mooring.yaml");
    write_mooring(&src, Some("days since 1950-01-01"), None);

    handle_dispatch(&["header".into(), "generic".into(), s(&src), s(&dest)]).unwrap();
    let yaml: serde_yaml::Value = serde_yaml::from_reader(File::open(&dest).unwrap()).unwrap();
    let h = &yaml["mooring"];
    assert_eq!(h["global_attributes"]["title"], "Test mooring");
    assert_eq!(h["global_attributes"]["station_id"], "42");
    assert_eq!(h["dimensions"]["z"], 4);
    assert_eq!(h["variables"]["sal"]["dimensions"][1], "z");
}

/// The cruise file with its platform held in a `string [cast]` or `int [cast]`
/// variable instead of `char [cast, strlen]`.
fn write_cruise_platform(path: &Path, kind: &str) {
    write_cruise(path);
    let mut f = netcdf::append(path).unwrap();
    match kind {
        "string" => {
            let mut v = f.add_variable_with_type("ship_s", &["cast"], &NcVariableType::String).unwrap();
            v.put_string("RV ONE", [0]).unwrap();
            v.put_string("RV TWO", [1]).unwrap();
        }
        _ => {
            let mut v = f.add_variable::<i32>("ship_s", &["cast"]).unwrap();
            v.put_values(&[6_900_001i32, 6_900_002], ..).unwrap();
        }
    }
}

#[test]
fn platform_from_string_and_integer_variables() {
    for (kind, expected) in [("string", ["RV ONE", "RV TWO"]), ("int", ["6900001", "6900002"])] {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("cruise.nc");
        let dest = dir.path().join("cruise.parquet");
        write_cruise_platform(&src, kind);
        let map = write_map(dir.path(), "map.toml", &CRUISE_MAP.replace("variable = \"ship\"", "variable = \"ship_s\""));

        convert(Some(&map), &[], &src, &dest).unwrap();
        let p = strs(&read(&dest), "platform_code");
        assert_eq!(p[0], expected[0], "{kind}");
        assert_eq!(p[3], expected[1], "{kind}");
    }
}

/// The cruise file with packed latitude, a filled longitude and a char time QC.
fn write_cruise_cf_coords(path: &Path) {
    let mut f = netcdf::create(path).unwrap();
    f.add_dimension("cast", 2).unwrap();
    f.add_dimension("level", 3).unwrap();

    let mut t = f.add_variable::<f64>("time", &["cast"]).unwrap();
    t.put_attribute("units", "days since 2000-01-01").unwrap();
    t.put_values(&[0.5, 1.0], ..).unwrap();

    // Packed: value = raw * 0.01 + 50.
    let mut lat = f.add_variable::<i16>("lat", &["cast"]).unwrap();
    lat.put_attribute("scale_factor", 0.01f64).unwrap();
    lat.put_attribute("add_offset", 50.0f64).unwrap();
    lat.put_values(&[1000i16, 1150], ..).unwrap();

    let mut lon = f.add_variable::<f64>("lon", &["cast"]).unwrap();
    lon.set_fill_value(-999.0f64).unwrap();
    lon.put_values(&[20.0, -999.0], ..).unwrap();

    let mut tqc = f.add_variable_with_type("tqc", &["cast"], &NcVariableType::Char).unwrap();
    tqc.put_values(&chars("4 ", 2), ..).unwrap();

    let mut depth = f.add_variable::<f32>("depth", &["cast", "level"]).unwrap();
    depth.put_values(&[5.0f32, 10.0, 15.0, 5.0, 10.0, 15.0], ..).unwrap();
    let mut temp = f.add_variable::<f32>("TEMP", &["cast", "level"]).unwrap();
    temp.put_values(&[5.0f32, 4.5, 4.0, 6.0, 5.5, 5.0], ..).unwrap();
}

#[test]
fn coordinates_are_cf_decoded_and_char_qc_read() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("cruise.nc");
    let dest = dir.path().join("cruise.parquet");
    write_cruise_cf_coords(&src);
    let map = format!("{CRUISE_MAP}\n").replace("deph = \"depth\"", "deph = \"depth\"\ntime_qc = \"tqc\"");
    let map = write_map(dir.path(), "map.toml", &map.replace("[platform]\nvariable = \"ship\"\n", ""));

    convert(Some(&map), &[], &src, &dest).unwrap();
    let df = read(&dest);
    assert_eq!(df.height(), 6);

    // Packed latitude is unpacked, per profile and tiled over its levels.
    let lat = f64s(&df, "latitude");
    assert!((lat[0] - 60.0).abs() < 1e-4 && (lat[2] - 60.0).abs() < 1e-4, "{lat:?}");
    assert!((lat[3] - 61.5).abs() < 1e-4, "{lat:?}");
    // A longitude equal to its _FillValue becomes NaN, not -999.
    let lon = f64s(&df, "longitude");
    assert_eq!(lon[0], 20.0);
    assert!(lon[3..].iter().all(|v| v.is_nan()), "{lon:?}");
    // Char time QC: a blank flag is empty.
    assert_eq!(strs(&df, "time_qc"), ["4", "4", "4", "", "", ""]);
}

#[test]
fn platform_from_scalar_variables_and_platform_number_default() {
    // Each variant adds a platform variable to the cruise file; `None` as the
    // mapping's variable means "no [platform] section" (the profile-level
    // default, PLATFORM_NUMBER).
    let cases: [(&str, Option<&str>, [&str; 2]); 4] = [
        ("scalar_int", Some("station"), ["7", "7"]),
        ("scalar_string", Some("station"), ["RV SOLO", "RV SOLO"]),
        ("char_1d", Some("station"), ["BUOY9", "BUOY9"]),
        ("platform_number", None, ["ARGO1", "ARGO2"]),
    ];
    for (kind, var, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("cruise.nc");
        let dest = dir.path().join("cruise.parquet");
        write_cruise(&src);
        {
            let mut f = netcdf::append(&src).unwrap();
            match kind {
                "scalar_int" => {
                    let mut v = f.add_variable::<i32>("station", &[]).unwrap();
                    v.put_values(&[7i32], ..).unwrap();
                }
                "scalar_string" => {
                    let mut v = f.add_variable_with_type("station", &[], &NcVariableType::String).unwrap();
                    v.put_string(" RV SOLO ", ..).unwrap();
                }
                "char_1d" => {
                    let mut v = f.add_variable_with_type("station", &["strlen"], &NcVariableType::Char).unwrap();
                    v.put_values(&chars("BUOY9", 8), ..).unwrap();
                }
                _ => {
                    let mut v = f
                        .add_variable_with_type("PLATFORM_NUMBER", &["cast", "strlen"], &NcVariableType::Char)
                        .unwrap();
                    let mut names = chars("ARGO1", 8);
                    names.extend(chars("ARGO2", 8));
                    v.put_values(&names, ..).unwrap();
                }
            }
        }
        let platform = match var {
            Some(v) => format!("[platform]\nvariable = \"{v}\"\n"),
            None => String::new(),
        };
        let map = write_map(
            dir.path(),
            "map.toml",
            &CRUISE_MAP.replace("[platform]\nvariable = \"ship\"\n", &platform),
        );

        convert(Some(&map), &[], &src, &dest).unwrap();
        let p = strs(&read(&dest), "platform_code");
        assert_eq!([p[0].as_str(), p[3].as_str()], expected, "{kind}");
    }
}

#[test]
fn platform_from_named_global_attribute() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mooring.nc");
    let dest = dir.path().join("mooring.parquet");
    write_mooring(&src, Some("days since 1950-01-01"), None);
    let map = MOORING_MAP.replace("platform_code = \"MOOR1\"\n", "") + "\n[platform]\nattribute = \"station_id\"\n";
    let map = write_map(dir.path(), "map.toml", &map);

    convert(Some(&map), &[], &src, &dest).unwrap();
    assert!(strs(&read(&dest), "platform_code").iter().all(|p| p == "42"));
}

#[test]
fn invalid_mappings_are_errors() {
    let mooring = |p: &Path| write_mooring(p, Some("days since 1950-01-01"), None);
    let cases = [
        (MOORING_MAP.replace("data_mode = \"D\"", "data_mode = \"X\""), "data_mode must be \"R\", \"A\" or \"D\""),
        (MOORING_MAP.replace("longitude = 20.0", "longitude = 400.0"), "[defaults] longitude 400 is out of range"),
        (format!("{MOORING_MAP}latitude = -95.0\n"), "[defaults] latitude -95 is out of range"),
        (MOORING_MAP.replace("time = \"time\"", "time = \"t\""), "time variable \"t\" not found"),
        (MOORING_MAP.replace("pres = \"pressure\"", "pres = \"P\""), "neither pressure \"P\" nor depth"),
        (MOORING_MAP.replace("temp = \"temperature\"", "temp = \"time\""), "variable \"time\" has dimensions [\"time\"]; expected [\"time\", \"z\"]"),
    ];
    for (map, needle) in cases {
        expect_error(&map, mooring, needle);
    }
}
