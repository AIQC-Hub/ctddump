use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

use serde::{Deserialize, Serialize};

use crate::Config;
use crate::convert::common;
use super::common as header_common;

/// Same shape as the NRT header, so `concat header` and `report yaml` treat both
/// alike; user files have no fixed attribute set, so every global attribute is kept.
#[derive(Serialize, Deserialize, Debug)]
struct NetCDFHeader {
    global_attributes: HashMap<String, String>,
    dimensions: HashMap<String, usize>,
    variables: HashMap<String, header_common::VariableMetadata>,
}

/// Extract metadata from a single NetCDF file to YAML. Called by both `run` and the batch processor.
pub fn extract_file(src: &str, dest: &str) -> Result<(), Box<dyn Error>> {
    let file = netcdf::open(src).map_err(|e| format!("{src}: {e}"))?;
    let header = NetCDFHeader {
        global_attributes: header_common::collect_all_global_attributes(&file),
        dimensions: header_common::collect_dimensions(&file),
        variables: header_common::collect_variables_and_metadata(&file),
    };
    let mut metadata: HashMap<String, NetCDFHeader> = HashMap::new();
    metadata.insert(common::get_base_file_name(src)?, header);
    serde_yaml::to_writer(File::create(dest)?, &metadata)?;
    Ok(())
}

pub fn run(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let [src, dest] = args else {
        return Err("expected a source and a destination".into());
    };
    extract_file(src, dest)?;
    Ok(Config { module: "header".to_string(), target: "generic".to_string(), args: args.to_vec() })
}
