use std::fmt;
use std::error::Error;

use super::Config;

pub mod common;
pub mod common_head;
pub mod nrt_head;
pub mod nrt_ar;
pub mod nrt_bo;
pub mod nrt_mo;
pub mod nrt_gl;
pub mod cora_head;
pub mod cora;
pub mod cora_legacy;

#[derive(Debug)]
struct UnknownTargetError;

impl fmt::Display for UnknownTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown target")
    }
}

impl Error for UnknownTargetError {}

#[derive(Debug)]
enum Target {
    NrtHead,
    NrtAr,
    NrtBo,
    NrtMo,
    NrtGl,
    CoraHead,
    Cora,
    CoraLegacy,
}

impl Target {
    fn dispatch(&self, args: &[String]) -> Result<Config, Box<dyn Error>> {
        match self {
            Target::NrtHead => {
                nrt_head::run(args)
            },
            Target::NrtAr => {
                nrt_ar::run(args)
            },
            Target::NrtBo => {
                nrt_bo::run(args)
            },
            Target::NrtMo => {
                nrt_mo::run(args)
            },
            Target::NrtGl => {
                nrt_gl::run(args)
            },
            Target::CoraHead => {
                cora_head::run(args)
            },
            Target::Cora => {
                cora::run(args)
            },
            Target::CoraLegacy => {
                cora_legacy::run(args)
            },
        }
    }
}

fn parse_target(arg: &str) -> Option<Target> {
    match arg {
        "nrt_head" => Some(Target::NrtHead),
        "nrt_ar" => Some(Target::NrtAr),
        "nrt_bo" => Some(Target::NrtBo),
        "nrt_mo" => Some(Target::NrtMo),
        "nrt_gl" => Some(Target::NrtGl),
        "cora_head" => Some(Target::CoraHead),
        "cora" => Some(Target::Cora),
        "cora_legacy" => Some(Target::CoraLegacy),
        _ => None,
    }
}

pub fn handle_target_dispatch(args: &[String]) -> Result<Config, Box<dyn Error>> {
    // Expect at least one argument (the module name)
    if args.is_empty() {
        return Err("No target specified".into());
    }

    // Parse the first argument as the module name
    let target = parse_target(&args[0]);

    // Dispatch to the correct module
    match target {
        Some(t) => {
            // Pass remaining arguments (if any) to the module
            let target_args = &args[1..];
            t.dispatch(target_args)
        }
        None => Err(Box::new(UnknownTargetError)),
    }
}

// A struct to hold the configuration after parsing the arguments
#[derive(Debug)]
struct ConvertConfig {
    src_file: String,
    target_file: String,
}

impl ConvertConfig {
    pub fn build(args: &[String]) -> Result<ConvertConfig, Box<dyn Error>> {
        if args.len() < 2 {
            return Err("not enough arguments".into());
        }

        let src_file = args[0].clone();
        let target_file = args[1].clone();

        Ok(ConvertConfig {
            src_file,
            target_file,
        })
    }
}
