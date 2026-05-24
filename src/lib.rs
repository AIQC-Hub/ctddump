use std::error::Error;
use std::fmt;

pub mod netcdf;

#[derive(Debug)]
struct UnknownModuleError;

impl fmt::Display for UnknownModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown module")
    }
}

impl Error for UnknownModuleError {}

// A struct to hold the configuration after parsing the arguments
#[derive(Debug, PartialEq)]
pub struct Config {
    pub module: String,
    pub target: String,
    pub args: Vec<String>,
}

// Define the different modules as an enum
#[derive(Debug)]
enum Module {
    Convert,
    Concat,
}

// Implement dispatching logic for the modules
impl Module {
    fn dispatch(&self, args: &[String]) -> Result<Config, Box<dyn Error>> {
        match self {
            Module::Convert => {
                netcdf::handle_target_dispatch(args)
            }
            Module::Concat => {
                println!("Calling Concat module with arguments: {:?}", args);
                Ok(Config {
                    module: "concat".to_string(),
                    target: "".to_string(),
                    args: args.to_vec(),
                })
            }
        }
    }
}

// Parse the first command-line argument into the correct enum variant
fn parse_module(arg: &str) -> Option<Module> {
    match arg {
        "netcdf" => Some(Module::Convert),
        "concat" => Some(Module::Concat),
        _ => None,
    }
}

pub fn handle_dispatch(args: &[String]) -> Result<Config, Box<dyn Error>> {
    // Expect at least one argument (the module name)
    if args.is_empty() {
        return Err("No module specified".into());
    }

    // Parse the first argument as the module name
    let module = parse_module(&args[0]);

    // Dispatch to the correct module
    match module {
        Some(m) => {
            // Pass remaining arguments (if any) to the module
            let module_args = &args[1..];
            m.dispatch(module_args)
        }
        None => Err(Box::new(UnknownModuleError)),
    }
}

