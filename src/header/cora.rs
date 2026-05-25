use std::process;
use std::error::Error;
use std::collections::HashMap;
use std::fs::File;

use netcdf;
use serde::{Serialize, Deserialize};

use crate::Config;
use crate::convert::common;
use super::HeaderConfig;
use super::common as header_common;

#[derive(Serialize, Deserialize, Debug)]
struct NetCDFHeader {
    dimensions: HashMap<String, usize>,
    variables: HashMap<String, header_common::VariableMetadata>,
}

fn netcdf_to_yaml(config: &HeaderConfig) -> Result<(), Box<dyn std::error::Error>> {
    let src_file = &config.src_file;
    let target_file = &config.target_file;
    let mut metadata: HashMap<String, NetCDFHeader> = HashMap::new();

    let filename = common::get_base_file_name(src_file)?;

    let header = collect_netcdf_metadata(src_file)?;
    metadata.insert(filename, header);

    let yaml_file = File::create(target_file)?;
    serde_yaml::to_writer(yaml_file, &metadata)?;

    Ok(())
}

fn collect_netcdf_metadata(
    src_file: &str,
) -> Result<NetCDFHeader, Box<dyn std::error::Error>> {
    let file = netcdf::open(src_file)?;

    let dimensions = header_common::collect_dimensions(&file);
    let variables = header_common::collect_variables_and_metadata(&file);

    Ok(NetCDFHeader {
        dimensions,
        variables,
    })
}

/// Extract metadata from a single NetCDF file to YAML. Called by both `run` and the batch processor.
pub fn extract_file(src: &str, dest: &str) -> Result<(), Box<dyn Error>> {
    let config = HeaderConfig {
        src_file: src.to_string(),
        target_file: dest.to_string(),
    };
    netcdf_to_yaml(&config)
}

pub fn run(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let config = HeaderConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    match netcdf_to_yaml(&config) {
        Ok(_) => Ok(Config {
            module: "header".to_string(),
            target: "cora".to_string(),
            args: args.to_vec(),
        }),
        Err(e) => {
            eprintln!("Header extraction failed: {}", e);
            process::exit(1);
        }
    }
}
