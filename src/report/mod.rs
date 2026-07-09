//! `report` subcommand: summarise a Parquet data file or a YAML header file as a
//! text report (TSV, plain text, or JSON) written to a file or stdout.

pub mod format;
pub mod parquet;
pub mod yaml;
