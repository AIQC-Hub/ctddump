use std::error::Error;
use std::fmt;

pub mod grep;
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
    module: String,
    target: String,
    args: Vec<String>,
}

// Define the different modules as an enum
#[derive(Debug)]
enum Module {
    Grep,
    Convert,
    Concat,
}

// Implement dispatching logic for the modules
impl Module {
    fn dispatch(&self, args: &[String]) -> Result<Config, Box<dyn Error>> {
        match self {
            Module::Grep => {
                grep::run(args)
            }
            Module::Convert => {
                println!("{:?}", args);
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
        "grep" => Some(Module::Grep),
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

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_dispatch_grep() {
        // Simulate passing the 'grep' command with two arguments
        let args = vec!["grep".to_string(), "the".to_string(), "./tests/test_data/poem.txt".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'grep' command
        let expected = Config {
            module: "grep".to_string(),
            target: "".to_string(),
            args: vec!["the".to_string(), "./tests/test_data/poem.txt".to_string()],
        };

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_head() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_head".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_head".to_string(),
            args: vec!["./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.yaml".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_ar_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_ar".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_ar".to_string(),
            args: vec!["./tests/test_data/AR_PR_CT_ITP-71.nc".to_string(), "./tests/test_data/AR_PR_CT_ITP-71.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_ar_2() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_ar".to_string(), "./tests/test_data/AR_PR_CT_58KN.nc".to_string(), "./tests/test_data/AR_PR_CT_58KN.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_ar".to_string(),
            args: vec!["./tests/test_data/AR_PR_CT_58KN.nc".to_string(), "./tests/test_data/AR_PR_CT_58KN.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_bo_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.nc".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_bo".to_string(),
            args: vec!["./tests/test_data/BO_PR_CT_ARH160003.nc".to_string(), "./tests/test_data/BO_PR_CT_ARH160003.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }
    #[test]
    fn test_netcdf_nrt_bo_2() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.nc".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_bo".to_string(),
            args: vec!["./tests/test_data/BO_PR_CT_BRK5059505.nc".to_string(), "./tests/test_data/BO_PR_CT_BRK5059505.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_bo_3() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_bo".to_string(),
            args: vec!["./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHIHAVSTENSFJORD.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_bo_4() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_bo".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_bo".to_string(),
            args: vec!["./tests/test_data/BO_PR_CT_SMHI3125.nc".to_string(), "./tests/test_data/BO_PR_CT_SMHI3125.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_mo_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_mo".to_string(),
            args: vec!["./tests/test_data/MO_PR_CT_SicilyChannel_1990.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_1990.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_mo_2() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_mo".to_string(),
            args: vec!["./tests/test_data/MO_PR_CT_SicilyChannel_2017.nc".to_string(), "./tests/test_data/MO_PR_CT_SicilyChannel_2017.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_mo_3() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_mo".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.nc".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_mo".to_string(),
            args: vec!["./tests/test_data/MO_PR_CT_SardiniaChannel_2008.nc".to_string(), "./tests/test_data/MO_PR_CT_SardiniaChannel_2008.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_nrt_gl_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "nrt_gl".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.nc".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "nrt_gl".to_string(),
            args: vec!["./tests/test_data/GL_PR_CT_EXEC004K.nc".to_string(), "./tests/test_data/GL_PR_CT_EXEC004K.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_cora_head() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "cora_head".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "cora_head".to_string(),
            args: vec!["./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.yaml".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_cora_ar_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "cora".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "cora".to_string(),
            args: vec!["./tests/test_data/CO_DMQCGL01_20201010_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201010_PR_CT.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_cora_bo_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "cora".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "cora".to_string(),
            args: vec!["./tests/test_data/CO_DMQCGL01_20201005_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_20201005_PR_CT.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_netcdf_cora2_bo_1() {
        // Simulate passing the 'netcdf' command with two arguments
        let args = vec!["netcdf".to_string(), "cora2".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.parquet".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'netcdf' command
        let expected = Config {
            module: "netcdf".to_string(),
            target: "cora2".to_string(),
            args: vec!["./tests/test_data/CO_DMQCGL01_19861204_PR_CT.nc".to_string(), "./tests/test_data/CO_DMQCGL01_19861204_PR_CT.parquet".to_string()],
        };

        println!("{:?}", result);

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_handle_dispatch_concat() {
        // Simulate passing the 'concat' command with one argument
        let args = vec!["concat".to_string(), "--arg1=val1".to_string()];
        let result = handle_dispatch(&args);

        // Expected configuration for the 'concat' command
        let expected = Config {
            module: "concat".to_string(),
            target: "".to_string(),
            args: vec!["--arg1=val1".to_string()],
        };

        // Assert that the result matches the expected Config
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_handle_dispatch_unknown_module() {
        // Simulate passing an unknown module
        let args = vec!["unknown".to_string()];
        let result = handle_dispatch(&args);

        // Assert that the result is an error
        if let Err(e) = result {
            assert_eq!(e.to_string(), "Unknown module");
        }
    }

    #[test]
    fn test_handle_dispatch_no_module() {
        // Simulate no module being passed
        let args: Vec<String> = vec![];
        let result = handle_dispatch(&args);

        // Assert that the result is an error for no module
        if let Err(e) = result {
            assert_eq!(e.to_string(), "No module specified");
        }
    }
}
