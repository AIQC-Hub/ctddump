use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

/// Configuration that describes the structure of an NRT NetCDF source file.
/// All NRT regions share the same conversion logic; only these flags differ.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NrtConfig {
    /// Whether the source file contains a DEPH variable.
    /// If `false`, depth is derived from pressure via TEOS-10 conversion.
    pub has_deph_source: bool,
    /// Whether to derive `profile_longitude` / `profile_latitude` columns.
    ///
    /// When `true`, the converter looks for profile-level position sources in order:
    ///   1. `PRECISE_LONGITUDE` / `PRECISE_LATITUDE` — used as-is if present.
    ///   2. `DEPLOY_LONGITUDE` / `DEPLOY_LATITUDE` — expanded from the `DEPLOYMENT`
    ///      time-index dimension (0-based) if present.
    ///   3. NaN — if neither source exists.
    ///
    /// The standard `LONGITUDE` / `LATITUDE` columns are always written.
    /// Both coordinate pairs NaN-fill each other: if one is NaN at a given
    /// observation, the value from the other pair is used as a fallback.
    /// `PRECISE_*` takes priority over `DEPLOY_*` if both are present.
    pub has_profile_coords: bool,
}

impl NrtConfig {
    /// Arctic Sea (AR): PRES-only source, standard coordinates.
    pub fn nrt_ar() -> Self {
        Self { has_deph_source: false, has_profile_coords: false }
    }
    /// Baltic Sea (BO): DEPH source present, profile coordinates enabled.
    pub fn nrt_bo() -> Self {
        Self { has_deph_source: true, has_profile_coords: true }
    }
    /// Mediterranean Sea (MO): PRES-only source, standard coordinates.
    pub fn nrt_mo() -> Self {
        Self { has_deph_source: false, has_profile_coords: false }
    }
    /// Global (GL): DEPH source present, standard coordinates.
    pub fn nrt_gl() -> Self {
        Self { has_deph_source: true, has_profile_coords: false }
    }

    /// Load config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
