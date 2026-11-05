use std::error::Error;

pub mod common;
pub mod nrt_config;
pub mod nrt;
pub mod cora_config;
pub mod cora;
pub mod generic_config;
pub mod generic;
pub mod source;
pub mod time_units;

#[derive(Debug)]
pub struct ConvertConfig {
    pub src_file: String,
    pub target_file: String,
}

impl ConvertConfig {
    pub fn build(args: &[String]) -> Result<ConvertConfig, Box<dyn Error>> {
        if args.len() < 2 {
            return Err("not enough arguments".into());
        }

        Ok(ConvertConfig {
            src_file: args[0].clone(),
            target_file: args[1].clone(),
        })
    }
}
