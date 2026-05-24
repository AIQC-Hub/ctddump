use std::fs;
use std::env;
use std::process;
use std::error::Error;
use super::Config;

// A struct to hold the configuration after parsing the arguments
#[derive(Debug)]
struct GrepConfig {
    query: String,
    file_path: String,
    ignore_case: bool,
}

impl GrepConfig {
    pub fn build(args: &[String]) -> Result<GrepConfig, Box<dyn Error>> {
        if args.len() < 2 {
             return Err("not enough arguments".into());
        }

        let query = args[0].clone();
        let file_path = args[1].clone();

        let ignore_case = env::var("IGNORE_CASE").is_ok();

        Ok(GrepConfig {
            query,
            file_path,
            ignore_case,
        })
    }
}

fn search<'a>(query: &str, contents: &'a str) -> Vec<&'a str> {
    let mut results = Vec::new();

    for line in contents.lines() {
        if line.contains(query) {
            results.push(line);
        }
    }

    results
}

fn search_case_insensitive<'a>(query: &str, contents: &'a str) -> Vec<&'a str> {
    let query = query.to_lowercase();
    let mut results = Vec::new();

    for line in contents.lines() {
        if line.to_lowercase().contains(&query) {
            results.push(line);
        }
    }

    results
}

pub fn run(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let config = GrepConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    let contents = fs::read_to_string(config.file_path)?;

    let results = if config.ignore_case {
        search_case_insensitive(&config.query, &contents)
    } else {
        search(&config.query, &contents)
    };

    for line in results {
        println!("{line}");
    }

    Ok(Config {
        module: "grep".to_string(),
        target: "".to_string(),
        args: args.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_sensitive() {
        let query = "duct";
        let contents = "\
Rust:
safe, fast, productive.
Pick three.
Duct tape.";

        assert_eq!(vec!["safe, fast, productive."], search(query, contents));
    }

    #[test]
    fn case_insensitive() {
        let query = "rUsT";
        let contents = "\
Rust:
safe, fast, productive.
Pick three.
Trust me.";

        assert_eq!(
            vec!["Rust:", "Trust me."],
            search_case_insensitive(query, contents)
        );
    }
}
