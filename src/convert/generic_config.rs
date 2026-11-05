use serde::{Deserialize, Serialize};

use super::cora_config::QcType;

/// How profiles and levels are arranged in a `generic` source file.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// One platform per file, measurements on `[profile, level]` with the
    /// profile dimension indexed by time (Copernicus NRT: `TIME` x `DEPTH`).
    #[default]
    TimeDepth,
    /// Many profiles, possibly from several platforms, on `[profile, level]`
    /// (Copernicus CORA / Argo: `N_PROF` x `N_LEVELS`).
    ProfileLevel,
}

impl Layout {
    pub fn default_profile_dim(self) -> &'static str {
        match self {
            Layout::TimeDepth => "TIME",
            Layout::ProfileLevel => "N_PROF",
        }
    }

    pub fn default_level_dim(self) -> &'static str {
        match self {
            Layout::TimeDepth => "DEPTH",
            Layout::ProfileLevel => "N_LEVELS",
        }
    }
}

/// Dimension names. Unset fields take the layout's Copernicus default.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Dimensions {
    pub profile: Option<String>,
    pub level: Option<String>,
}

/// Variable names. Unset fields take the Copernicus name; `<param>_qc` and
/// `<param>_dm` default to the parameter name plus `_QC` / `_DM`. An empty
/// string means "the file has no such variable".
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Variables {
    pub time: Option<String>,
    pub longitude: Option<String>,
    pub latitude: Option<String>,
    pub time_qc: Option<String>,
    pub position_qc: Option<String>,
    pub temp: Option<String>,
    pub temp_qc: Option<String>,
    pub temp_dm: Option<String>,
    pub psal: Option<String>,
    pub psal_qc: Option<String>,
    pub psal_dm: Option<String>,
    pub pres: Option<String>,
    pub pres_qc: Option<String>,
    pub pres_dm: Option<String>,
    pub deph: Option<String>,
    pub deph_qc: Option<String>,
    pub deph_dm: Option<String>,
    /// Per-profile data mode (`profile-level` layout).
    pub data_mode: Option<String>,
}

/// Where the platform code is read from: at most one of the two.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Platform {
    /// Global attribute holding one platform code for the whole file.
    pub attribute: Option<String>,
    /// Per-profile variable (char, string or integer), or a scalar one.
    pub variable: Option<String>,
}

/// Values used when the file does not provide them.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GenericDefaults {
    pub platform_code: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    /// `R`, `A` or `D`; used where the file has no data-mode information.
    pub data_mode: Option<String>,
}

/// Mapping file for the `generic` format: describes a user's NetCDF file in
/// terms of the Copernicus structure. Every field is optional; an empty file
/// describes a Copernicus NRT file.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GenericConfig {
    pub layout: Layout,
    /// Overrides the time variable's `units` attribute, e.g.
    /// `"seconds since 1970-01-01 00:00:00"`.
    pub time_units: Option<String>,
    /// QC storage type; detected per variable when unset.
    pub qc_type: Option<QcType>,
    /// Glob pattern for batch file selection (default `*.nc`).
    pub pattern: Option<String>,
    pub dimensions: Dimensions,
    pub variables: Variables,
    pub platform: Platform,
    pub defaults: GenericDefaults,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_is_all_defaults() {
        let cfg: GenericConfig = toml::from_str("").unwrap();
        assert_eq!(cfg, GenericConfig::default());
        assert_eq!(cfg.layout, Layout::TimeDepth);
    }

    #[test]
    fn full_file_parses() {
        let cfg: GenericConfig = toml::from_str(
            r#"
            layout = "profile-level"
            time_units = "seconds since 1970-01-01"
            qc_type = "char"
            pattern = "cast_*.nc"

            [dimensions]
            profile = "cast"
            level = "z"

            [variables]
            time = "time"
            temp = "temperature"
            temp_qc = ""

            [platform]
            variable = "ship"

            [defaults]
            platform_code = "SHIP1"
            latitude = 60.5
            data_mode = "D"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.layout, Layout::ProfileLevel);
        assert_eq!(cfg.qc_type, Some(QcType::Char));
        assert_eq!(cfg.dimensions.profile.as_deref(), Some("cast"));
        assert_eq!(cfg.variables.temp.as_deref(), Some("temperature"));
        assert_eq!(cfg.variables.temp_qc.as_deref(), Some(""));
        assert_eq!(cfg.platform.variable.as_deref(), Some("ship"));
        assert_eq!(cfg.defaults.latitude, Some(60.5));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        // A typo must not be silently ignored.
        assert!(toml::from_str::<GenericConfig>("layot = \"time-depth\"").is_err());
        assert!(toml::from_str::<GenericConfig>("[variables]\ntemperature = \"T\"").is_err());
    }

    #[test]
    fn bad_layout_is_rejected() {
        assert!(toml::from_str::<GenericConfig>("layout = \"grid\"").is_err());
    }
}
