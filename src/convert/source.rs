//! Where a converter finds each piece of information in a NetCDF file.
//!
//! The NRT and CORA converters used to hardcode every dimension and variable
//! name. They now read them from a [`SourceSpec`]: the built-in constructors
//! ([`SourceSpec::nrt`], [`SourceSpec::cora`]) reproduce the Copernicus names
//! exactly, and the `generic` format builds one from a user mapping file.

use super::cora_config::{CoraConfig, QcType};
use super::time_units::TimeScale;

/// Where the platform code comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformSource {
    /// A global (file-level) attribute, one platform per file.
    Attribute(String),
    /// A per-profile variable: `char [profile, strlen]`, `string [profile]`,
    /// or an integer `[profile]` (e.g. a WMO number).
    Variable(String),
}

/// One measured parameter and its companion QC and data-mode variables.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub qc: String,
    pub dm: String,
}

impl Param {
    /// `NAME`, `NAME_QC`, `NAME_DM`: the Copernicus naming convention.
    pub fn conventional(name: &str) -> Self {
        Self { name: name.to_string(), qc: format!("{name}_QC"), dm: format!("{name}_DM") }
    }
}

/// Values used when the source file does not provide them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Defaults {
    pub platform_code: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    /// Data mode used when a file has no data-mode variable or attribute.
    /// `None` means [`super::common::DM_DEFAULT`] (`"R"`).
    pub data_mode: Option<String>,
}

/// Names and decoding rules for one NetCDF source layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceSpec {
    /// Dimension that indexes profiles (`TIME` in NRT, `N_PROF` in CORA).
    pub profile_dim: String,
    /// Dimension that indexes levels within a profile (`DEPTH` / `N_LEVELS`).
    pub level_dim: String,
    pub platform: PlatformSource,
    pub time: String,
    pub longitude: String,
    pub latitude: String,
    /// `None`: the source has no usable time QC; the column is left empty.
    pub time_qc: Option<String>,
    pub position_qc: Option<String>,
    pub temp: Param,
    pub psal: Param,
    pub pres: Param,
    pub deph: Param,
    /// Per-profile data-mode variable (profile-level layout only).
    pub data_mode: String,
    /// QC storage type; `None` detects it from each variable's NetCDF type.
    pub qc_type: Option<QcType>,
    pub defaults: Defaults,
    /// Apply CF decoding: `_FillValue` / `missing_value` (or the NetCDF default
    /// fill) becomes NaN on every numeric variable including time and position,
    /// and `scale_factor` / `add_offset` are applied. Off for the Copernicus
    /// formats, whose output must not change.
    pub cf_decode: bool,
    /// Converts raw time values to days since 1950-01-01.
    pub time_scale: TimeScale,
}

impl SourceSpec {
    /// Copernicus NRT names (`TIME` x `DEPTH`, `platform_code` attribute).
    pub fn nrt() -> Self {
        Self {
            profile_dim: "TIME".to_string(),
            level_dim: "DEPTH".to_string(),
            platform: PlatformSource::Attribute("platform_code".to_string()),
            time: "TIME".to_string(),
            longitude: "LONGITUDE".to_string(),
            latitude: "LATITUDE".to_string(),
            time_qc: Some("TIME_QC".to_string()),
            position_qc: Some("POSITION_QC".to_string()),
            temp: Param::conventional("TEMP"),
            psal: Param::conventional("PSAL"),
            pres: Param::conventional("PRES"),
            deph: Param::conventional("DEPH"),
            data_mode: "DATA_MODE".to_string(),
            qc_type: Some(QcType::Int),
            defaults: Defaults::default(),
            cf_decode: false,
            time_scale: TimeScale::DAYS_SINCE_1950,
        }
    }

    /// Copernicus CORA names (`N_PROF` x `N_LEVELS`, `PLATFORM_NUMBER`), with the
    /// time variable, QC type and time-QC presence taken from `config`.
    pub fn cora(config: &CoraConfig) -> Self {
        let qc = |name: &str| config.has_time_qc.then(|| name.to_string());
        Self {
            profile_dim: "N_PROF".to_string(),
            level_dim: "N_LEVELS".to_string(),
            platform: PlatformSource::Variable("PLATFORM_NUMBER".to_string()),
            time: config.time_var.clone(),
            time_qc: qc("TIME_QC"),
            position_qc: qc("POSITION_QC"),
            qc_type: Some(config.qc_type.clone()),
            ..Self::nrt()
        }
    }

    /// The data mode to use when a file carries none.
    pub fn dm_fallback(&self) -> &str {
        self.defaults.data_mode.as_deref().unwrap_or(super::common::DM_DEFAULT)
    }
}
