use std::error::Error;

pub mod common;
pub mod nrt;
pub mod cora;

#[derive(Debug)]
pub struct HeaderConfig {
    pub src_file: String,
    pub target_file: String,
}

impl HeaderConfig {
    pub fn build(args: &[String]) -> Result<HeaderConfig, Box<dyn Error>> {
        if args.len() < 2 {
            return Err("not enough arguments".into());
        }
        Ok(HeaderConfig {
            src_file: args[0].clone(),
            target_file: args[1].clone(),
        })
    }
}
