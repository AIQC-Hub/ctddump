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
    /// Whether to use PRECISE_LONGITUDE / PRECISE_LATITUDE instead of
    /// LONGITUDE / LATITUDE for position output and pressure↔depth conversion.
    pub has_precise_coords: bool,
}

impl NrtConfig {
    /// Arctic Sea (AR): PRES-only source, standard coordinates.
    pub fn nrt_ar() -> Self {
        Self { has_deph_source: false, has_precise_coords: false }
    }
    /// Baltic Sea (BO): DEPH source present, precise coordinates.
    pub fn nrt_bo() -> Self {
        Self { has_deph_source: true, has_precise_coords: true }
    }
    /// Mediterranean Sea (MO): PRES-only source, standard coordinates.
    pub fn nrt_mo() -> Self {
        Self { has_deph_source: false, has_precise_coords: false }
    }
    /// Global (GL): DEPH source present, standard coordinates.
    pub fn nrt_gl() -> Self {
        Self { has_deph_source: true, has_precise_coords: false }
    }

    /// Load config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
