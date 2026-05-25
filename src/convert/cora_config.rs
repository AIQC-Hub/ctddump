use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

/// How QC flags are stored in the NetCDF source file.
/// All formats produce `String` in the output; both storage types are converted
/// to single-character strings (e.g., `"1"`, `"4"`, `"A"`). Missing → `""`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum QcType {
    /// QC flags are stored as bytes (`i8`). Digits 0–9 are written as `"0"`–`"9"`.
    Int,
    /// QC flags are stored as ASCII characters ('0'–'9', 'A', 'B', …).
    /// Each character is written as a one-character string verbatim.
    Char,
}

/// Configuration that describes the structure of a CORA NetCDF source file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CoraConfig {
    /// Name of the time variable (`"TIME"` for current format, `"JULD"` for legacy).
    pub time_var: String,
    /// Storage type of QC flags in the source file.
    pub qc_type: QcType,
    /// Whether the source file contains TIME_QC and POSITION_QC variables.
    pub has_time_qc: bool,
    /// Whether the source file contains a DEPH variable.
    /// If `false`, only PRES is present and depth is not converted.
    pub has_deph_source: bool,
    /// Glob pattern matched against filenames (not full paths) during batch processing.
    /// `None` means "use the subcommand's built-in default" (`"*.nc"` for both CORA formats).
    #[serde(default)]
    pub pattern: Option<String>,
}

impl CoraConfig {
    /// Current CORA format: integer QC, TIME variable, DEPH present.
    pub fn cora() -> Self {
        Self {
            time_var: "TIME".to_string(),
            qc_type: QcType::Int,
            has_time_qc: true,
            has_deph_source: true,
            pattern: None,
        }
    }

    /// Legacy CORA format: char QC, JULD variable, no DEPH.
    pub fn cora_legacy() -> Self {
        Self {
            time_var: "JULD".to_string(),
            qc_type: QcType::Char,
            has_time_qc: false,
            has_deph_source: false,
            pattern: None,
        }
    }

    /// Load config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
