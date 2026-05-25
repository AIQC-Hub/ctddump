use std::error::Error;

pub mod common;
pub mod common_head;
pub mod nrt_config;
pub mod nrt;
pub mod nrt_head;
pub mod cora_config;
pub mod cora;
pub mod cora_head;

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
