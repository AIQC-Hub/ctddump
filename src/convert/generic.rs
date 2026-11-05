//! The `generic` format: user-provided NetCDF files described by a mapping file
//! ([`GenericConfig`]). A file is checked against the mapping up front so that
//! a mismatch is reported as a clear error naming the variable, not a panic or
//! a low-level NetCDF error halfway through the conversion.

use std::error::Error;

use crate::Config;
use super::common;
use super::cora;
use super::generic_config::{GenericConfig, Layout};
use super::nrt;
use super::nrt_config::NrtConfig;
use super::source::{Defaults, Param, PlatformSource, SourceSpec};
use super::time_units::TimeScale;

/// Calendars whose day length is fixed, so `<unit> since <date>` maps linearly
/// onto the output time base.
const SUPPORTED_CALENDARS: &[&str] = &["standard", "gregorian", "proleptic_gregorian"];

fn dims_of(file: &netcdf::File, var_name: &str) -> Option<Vec<String>> {
    file.variable(var_name).map(|v| v.dimensions().iter().map(|d| d.name()).collect())
}

/// `Some(name)` for a configured or default name, `None` for an explicit `""`.
fn name_or(value: &Option<String>, default: &str) -> Option<String> {
    match value.as_deref() {
        Some("") => None,
        Some(v) => Some(v.to_string()),
        None => Some(default.to_string()),
    }
}

impl GenericConfig {
    /// Resolve the mapping against `file` and validate that the file has what a
    /// conversion needs: the two dimensions, a time variable with usable units,
    /// a position (variable or default), a platform code (attribute, variable or
    /// default), at least one of pressure/depth and of temperature/salinity, and
    /// the expected shapes.
    pub fn resolve(&self, file: &netcdf::File) -> Result<SourceSpec, Box<dyn Error>> {
        let layout = self.layout;
        let v = &self.variables;
        let profile_dim = self.dimensions.profile.clone().unwrap_or_else(|| layout.default_profile_dim().to_string());
        let level_dim = self.dimensions.level.clone().unwrap_or_else(|| layout.default_level_dim().to_string());

        for dim in [&profile_dim, &level_dim] {
            if file.dimension(dim).is_none() {
                let found: Vec<String> = file.dimensions().map(|d| d.name()).collect();
                return Err(format!(
                    "dimension \"{dim}\" not found (the file has: {}); set [dimensions] profile/level",
                    found.join(", ")
                ).into());
            }
        }

        // Per-profile variables are scalars or 1-D along the profile dimension;
        // measurements are 2-D [profile, level].
        let per_profile = |name: &str| -> Result<(), Box<dyn Error>> {
            match dims_of(file, name) {
                Some(d) if d.is_empty() || d == [profile_dim.clone()] => Ok(()),
                Some(d) => Err(format!("variable \"{name}\" has dimensions {d:?}; expected [] or [\"{profile_dim}\"]").into()),
                None => Ok(()),
            }
        };
        let grid = |name: &str| -> Result<(), Box<dyn Error>> {
            match dims_of(file, name) {
                Some(d) if d == [profile_dim.clone(), level_dim.clone()] => Ok(()),
                Some(d) => Err(format!(
                    "variable \"{name}\" has dimensions {d:?}; expected [\"{profile_dim}\", \"{level_dim}\"]"
                ).into()),
                None => Ok(()),
            }
        };
        let exists = |name: &Option<String>| name.as_deref().is_some_and(|n| file.variable(n).is_some());

        // Time
        let time = name_or(&v.time, "TIME").ok_or("variables.time cannot be empty: a time variable is required")?;
        if file.variable(&time).is_none() {
            return Err(format!("time variable \"{time}\" not found; set [variables] time").into());
        }
        per_profile(&time)?;
        let time_var = file.variable(&time).unwrap();
        let units = match &self.time_units {
            Some(u) => u.clone(),
            None => time_var
                .attribute_value("units")
                .and_then(|a| a.ok())
                .and_then(common::get_string_value)
                .ok_or_else(|| format!("time variable \"{time}\" has no units attribute; set time_units in the mapping file"))?,
        };
        let time_scale = TimeScale::parse(&units)?;
        if let Some(cal) = time_var.attribute_value("calendar").and_then(|a| a.ok()).and_then(common::get_string_value) {
            if !SUPPORTED_CALENDARS.contains(&cal.trim().to_ascii_lowercase().as_str()) {
                return Err(format!(
                    "time variable \"{time}\" uses calendar \"{cal}\"; only {} are supported",
                    SUPPORTED_CALENDARS.join(", ")
                ).into());
            }
        }

        // Position
        let defaults = &self.defaults;
        let longitude = name_or(&v.longitude, "LONGITUDE");
        let latitude = name_or(&v.latitude, "LATITUDE");
        for (name, default, what) in [(&longitude, defaults.longitude, "longitude"), (&latitude, defaults.latitude, "latitude")] {
            if exists(name) {
                per_profile(name.as_deref().unwrap())?;
            } else if default.is_none() {
                return Err(format!(
                    "{what} variable \"{}\" not found; set [variables] {what} or [defaults] {what}",
                    name.as_deref().unwrap_or("")
                ).into());
            }
        }
        if let Some(lon) = defaults.longitude {
            if !(-180.0..=360.0).contains(&lon) {
                return Err(format!("[defaults] longitude {lon} is out of range").into());
            }
        }
        if let Some(lat) = defaults.latitude {
            if !(-90.0..=90.0).contains(&lat) {
                return Err(format!("[defaults] latitude {lat} is out of range").into());
            }
        }

        // Measurements: names, then presence and shape.
        let param = |value: &Option<String>, qc: &Option<String>, dm: &Option<String>, default: &str| -> Option<Param> {
            let name = name_or(value, default)?;
            Some(Param {
                qc: name_or(qc, &format!("{name}_QC")).unwrap_or_default(),
                dm: name_or(dm, &format!("{name}_DM")).unwrap_or_default(),
                name,
            })
        };
        let absent = || Param { name: String::new(), qc: String::new(), dm: String::new() };
        let temp = param(&v.temp, &v.temp_qc, &v.temp_dm, "TEMP").unwrap_or_else(absent);
        let psal = param(&v.psal, &v.psal_qc, &v.psal_dm, "PSAL").unwrap_or_else(absent);
        let pres = param(&v.pres, &v.pres_qc, &v.pres_dm, "PRES").unwrap_or_else(absent);
        let deph = param(&v.deph, &v.deph_qc, &v.deph_dm, "DEPH").unwrap_or_else(absent);

        let present = |p: &Param| !p.name.is_empty() && file.variable(&p.name).is_some();
        if !present(&pres) && !present(&deph) {
            return Err(format!(
                "neither pressure \"{}\" nor depth \"{}\" found; set [variables] pres or deph",
                pres.name, deph.name
            ).into());
        }
        if !present(&temp) && !present(&psal) {
            return Err(format!(
                "neither temperature \"{}\" nor salinity \"{}\" found; set [variables] temp or psal",
                temp.name, psal.name
            ).into());
        }
        for p in [&temp, &psal, &pres, &deph] {
            for name in [&p.name, &p.qc] {
                if !name.is_empty() {
                    grid(name)?;
                }
            }
            if layout == Layout::TimeDepth && !p.dm.is_empty() {
                grid(&p.dm)?;
            }
        }

        // QC and data mode
        let time_qc = name_or(&v.time_qc, "TIME_QC");
        let position_qc = name_or(&v.position_qc, "POSITION_QC");
        for name in [&time_qc, &position_qc].into_iter().flatten() {
            per_profile(name)?;
        }
        let data_mode = name_or(&v.data_mode, "DATA_MODE").unwrap_or_default();
        if layout == Layout::ProfileLevel && !data_mode.is_empty() {
            per_profile(&data_mode)?;
        }
        if let Some(dm) = &defaults.data_mode {
            if !matches!(dm.as_str(), "R" | "A" | "D") {
                return Err(format!("[defaults] data_mode must be \"R\", \"A\" or \"D\", not \"{dm}\"").into());
            }
        }

        // Platform
        let platform = match (&self.platform.attribute, &self.platform.variable) {
            (Some(_), Some(_)) => return Err("[platform] takes either attribute or variable, not both".into()),
            (Some(a), None) => PlatformSource::Attribute(a.clone()),
            (None, Some(var)) => PlatformSource::Variable(var.clone()),
            (None, None) => match layout {
                Layout::TimeDepth => PlatformSource::Attribute("platform_code".to_string()),
                Layout::ProfileLevel => PlatformSource::Variable("PLATFORM_NUMBER".to_string()),
            },
        };
        if defaults.platform_code.is_none() {
            let found = match &platform {
                PlatformSource::Attribute(a) => file.attribute(a).is_some(),
                PlatformSource::Variable(var) => file.variable(var).is_some(),
            };
            if !found {
                let (kind, name) = match &platform {
                    PlatformSource::Attribute(a) => ("global attribute", a),
                    PlatformSource::Variable(var) => ("variable", var),
                };
                return Err(format!(
                    "platform {kind} \"{name}\" not found; set [platform] attribute/variable or [defaults] platform_code"
                ).into());
            }
        }

        Ok(SourceSpec {
            profile_dim,
            level_dim,
            platform,
            time,
            longitude: longitude.unwrap_or_default(),
            latitude: latitude.unwrap_or_default(),
            time_qc,
            position_qc,
            temp,
            psal,
            pres,
            deph,
            data_mode,
            qc_type: self.qc_type.clone(),
            defaults: Defaults {
                platform_code: defaults.platform_code.clone(),
                longitude: defaults.longitude,
                latitude: defaults.latitude,
                data_mode: defaults.data_mode.clone(),
            },
            cf_decode: true,
            time_scale,
        })
    }
}

/// Convert a single user NetCDF file to Parquet. Called by both `run` and the
/// batch processor. Errors name the source file.
pub fn convert_file(src: &str, dest: &str, config: &GenericConfig) -> Result<(), Box<dyn Error>> {
    let file = netcdf::open(src).map_err(|e| format!("{src}: {e}"))?;
    let spec = config.resolve(&file).map_err(|e| format!("{src}: {e}"))?;
    let basename = common::get_base_file_name(src)?;
    match config.layout {
        Layout::TimeDepth => {
            // DEPH is auto-detected by the converter; PRECISE_*/DEPLOY_* profile
            // coordinates are a Copernicus NRT convention and stay off.
            let nrt_config = NrtConfig { has_deph_source: false, has_profile_coords: false, pattern: None };
            nrt::write_parquet(&file, &basename, dest, &nrt_config, &spec)
        }
        // `true`: always run the PRES <-> DEPH conversion, so a file with only one
        // of them still gets both columns (missing DEPH reads as all-NaN).
        Layout::ProfileLevel => cora::write_parquet(&file, &basename, dest, true, &spec),
    }
    .map_err(|e| format!("{src}: {e}").into())
}

pub fn run(src: &str, dest: &str, config: &GenericConfig) -> Result<Config, Box<dyn Error>> {
    convert_file(src, dest, config)?;
    Ok(Config {
        module: "convert".to_string(),
        target: "generic".to_string(),
        args: vec![src.to_string(), dest.to_string()],
    })
}
