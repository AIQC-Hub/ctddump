use std::env;

fn main() {
    // Collect command-line arguments
    let args: Vec<String> = env::args().skip(1).collect(); // skip the program name

    // Call the new dispatch function and handle the result
    match ctddump::handle_dispatch(&args) {
        Ok(config) => {
            println!("Successfully processed: {:?}", config);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Available modules: grep, netcdf, concat");
        }
    }
}
