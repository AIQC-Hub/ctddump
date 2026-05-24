use std::collections::HashMap;
use netcdf;
use serde::{Serialize, Deserialize};

use super::common;

#[derive(Serialize, Deserialize, Debug)]
pub struct VariableMetadata {
    pub data_type: String,
    pub dimensions: Vec<String>,
}

// Function to collect dimensions
pub fn collect_dimensions(file: &netcdf::File) -> HashMap<String, usize> {
    let mut dimensions = HashMap::new();
    for dim in file.dimensions() {
        let name = dim.name();
        let len = dim.len();
        dimensions.insert(name.to_string(), len);
    }
    dimensions
}

// Function to collect global attributes
pub fn collect_global_attributes(file: &netcdf::File, attr_names: &[&str]) -> HashMap<String, String> {
    let mut global_attributes = HashMap::new();

    for &attr_name in attr_names {
        let attr_val = match file.attribute(attr_name) {
            Some(attr) => {
                // get_string_value returns Option<String>
                common::get_string_value(attr.value().unwrap()).unwrap_or_else(|| "".to_string())
            }
            None => "".to_string(), // Attribute missing
        };

        global_attributes.insert(attr_name.to_string(), attr_val);
    }

    global_attributes
}

// Function to collect variables and metadata
pub fn collect_variables_and_metadata(file: &netcdf::File) -> HashMap<String, VariableMetadata> {
    let mut variables = HashMap::new();
    for var in file.variables() {
        let name = var.name();
        variables.insert(name.to_string(), VariableMetadata {
            data_type: format!("{:?}", var.vartype()),
            dimensions: var.dimensions().iter().map(|d| d.name().to_string()).collect(),
        });
    }
    variables
}
