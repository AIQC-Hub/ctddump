use std::process;
use std::error::Error;
use std::collections::HashMap;
use std::fs::File;

use netcdf;
use serde::{Serialize, Deserialize};

use crate::Config;
use super::ConvertConfig;
use super::common;
use super::common_head;

#[derive(Serialize, Deserialize, Debug)]
struct NetCDFHeader {
    dimensions: HashMap<String, usize>,
    variables: HashMap<String, common_head::VariableMetadata>,
}

fn netcdf_to_yaml(config: &ConvertConfig) -> Result<(), Box<dyn std::error::Error>> {
    let src_file = &config.src_file;
    let target_file = &config.target_file;
    let mut metadata: HashMap<String, NetCDFHeader> = HashMap::new();

    // Get the file name (without directories) and then remove the extension
    let filename = common::get_base_file_name(&src_file)?;

    // Extract dimensions and variables metadata
    let header = collect_netcdf_metadata(src_file)?;
    metadata.insert(filename, header);

    // Write metadata to YAML
    let yaml_file = File::create(target_file)?;
    serde_yaml::to_writer(yaml_file, &metadata)?;

    Ok(())
}

/// Collects metadata and data from the NetCDF file.
/// Returns a tuple containing the NetCDF metadata and a Polars DataFrame.
fn collect_netcdf_metadata (
    src_file: &String,
) -> Result<NetCDFHeader, Box<dyn std::error::Error>> {
    // Open the NetCDF file
    let file = netcdf::open(src_file)?;

    let dimensions = common_head::collect_dimensions(&file);
    let variables = common_head::collect_variables_and_metadata(&file);

    // Collect dimensions
    Ok(NetCDFHeader {
        dimensions,
        variables,
    })
}

pub fn run(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let config = ConvertConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    match netcdf_to_yaml(&config) {
        Ok(_config) => {
            Ok(Config {
                module: "netcdf".to_string(),
                target: "cora_head".to_string(),
                args: args.to_vec(),
            })
        }
        Err(e) => {
            eprintln!("Conversion failed: {}", e);
            process::exit(1);
        }
    }
}
