//! YAML report: for each source file in a merged header YAML, record which core
//! physical columns exist and which biogeochemical/biological measurement
//! variables are present.

use std::error::Error;
use std::path::Path;

use polars::prelude::*;

use crate::cli::ReportFormat;
use crate::report::format;

/// Core physical measurement variables, excluded from the BGC list.
const CORE: [&str; 4] = ["TEMP", "PSAL", "PRES", "DEPH"];

/// Summarise `src` and write the report in `format` to `dest` (or stdout).
pub fn run(format_: ReportFormat, src: &Path, dest: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let content = std::fs::read_to_string(src)
        .map_err(|e| format!("Cannot read {}: {}", src.display(), e))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("Cannot parse {}: {}", src.display(), e))?;
    let map = doc
        .as_mapping()
        .ok_or_else(|| format!("{}: expected a top-level YAML mapping", src.display()))?;

    let mut filename = Vec::new();
    let mut has_temp = Vec::new();
    let mut has_psal = Vec::new();
    let mut has_pres = Vec::new();
    let mut has_deph = Vec::new();
    let mut has_time = Vec::new();
    let mut has_position = Vec::new();
    let mut extra_params = Vec::new();

    for (key, value) in map {
        let name = key.as_str().unwrap_or("").to_string();
        let vars = value.get("variables");
        let present = |v: &str| vars.and_then(|m| m.get(v)).is_some();

        filename.push(name);
        has_temp.push(present("TEMP"));
        has_psal.push(present("PSAL"));
        has_pres.push(present("PRES"));
        has_deph.push(present("DEPH"));
        has_time.push(present("TIME"));
        has_position.push(present("POSITION_QC"));
        extra_params.push(detect_extra_params(vars).join(";"));
    }

    let df = DataFrame::new(vec![
        Series::new("filename".into(), filename),
        Series::new("has_temp".into(), has_temp),
        Series::new("has_psal".into(), has_psal),
        Series::new("has_pres".into(), has_pres),
        Series::new("has_deph".into(), has_deph),
        Series::new("has_time".into(), has_time),
        Series::new("has_position".into(), has_position),
        Series::new("extra_params".into(), extra_params),
    ])?;

    format::write_report(&df, format_, dest)?;
    Ok(())
}

/// Detect the extra measurement parameters by complement: a `Float` variable
/// dimensioned `(TIME, DEPTH)`, not a `_QC` flag and not a core physical
/// (`TEMP/PSAL/PRES/DEPH`). This captures biogeochemical/biological parameters
/// (DOXY, FLU2, TUR3, …) as well as other non-core measurements (CNDC, SVEL, …).
/// Returns sorted variable names.
fn detect_extra_params(vars: Option<&serde_yaml::Value>) -> Vec<String> {
    let Some(mapping) = vars.and_then(|v| v.as_mapping()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for (vk, vv) in mapping {
        let Some(vname) = vk.as_str() else { continue };
        if vname.ends_with("_QC") || CORE.contains(&vname) {
            continue;
        }
        let is_float = vv
            .get("data_type")
            .and_then(|d| d.as_str())
            .is_some_and(|d| d.starts_with("Float"));
        let dims_ok = vv
            .get("dimensions")
            .and_then(|d| d.as_sequence())
            .is_some_and(|seq| {
                let dims: Vec<&str> = seq.iter().filter_map(|x| x.as_str()).collect();
                dims == ["TIME", "DEPTH"]
            });
        if is_float && dims_ok {
            out.push(vname.to_string());
        }
    }
    out.sort();
    out
}
