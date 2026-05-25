use std::process;
use std::error::Error;

use netcdf;
use polars::prelude::*;

use crate::Config;
use super::ConvertConfig;
use super::common;

fn netcdf_to_parquet(config: &ConvertConfig) -> Result<(), Box<dyn std::error::Error>> {
    let src_file = &config.src_file;
    let target_file = &config.target_file;

    // Extract ar data
    let df_ar_data = collect_netcdf_cora_data(src_file)?;

    // Filter NAN rows
    let mask = df_ar_data.column("temp")?.is_not_nan()?
        | df_ar_data.column("psal")?.is_not_nan()?
        | df_ar_data.column("pres")?.is_not_nan()?;
    let mut filtered_df = df_ar_data.filter(&mask)?;

    // Write Parguet file
    let mut out_file = std::fs::File::create(target_file).unwrap();
    ParquetWriter::new(&mut out_file).finish(&mut filtered_df).unwrap();

    Ok(())
}

fn collect_netcdf_cora_data(
    src_file: &String,
) -> Result<DataFrame, Box<dyn std::error::Error>> {

    // Open the NetCDF file
    let file = netcdf::open(src_file)?;

    // Dimensions
    let n_profs = file.dimension("N_PROF").unwrap().len();
    let n_levels = file.dimension("N_LEVELS").unwrap().len();
    let string32 = file.dimension("STRING32").unwrap().len();

    let vec_size = n_profs * n_levels;

    // Keys
    let platform_code: Vec<String> = common::get_char_value2(&file, "PLATFORM_NUMBER".to_string(), n_levels, n_profs, string32)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence(n_profs, n_levels)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(n_profs, n_levels)?;

    // Meta info
    let time: Vec<f64> = common::get_coordinate_value(&file, "JULD".to_string(), n_profs, n_levels, f64::NAN)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;
    let longitude: Vec<f64> = common::get_coordinate_value(&file, "LONGITUDE".to_string(), n_profs, n_levels, f64::NAN)?;
    let latitude: Vec<f64> = common::get_coordinate_value(&file, "LATITUDE".to_string(), n_profs, n_levels, f64::NAN)?;
    let basename = common::get_base_file_name(&src_file)?;
    let filename: Vec<String> = vec![basename.clone(); vec_size];

    // Temp
    let temp_fill_value: f32 = common::get_float_fill_value(&file, "TEMP".to_string());
    let temp: Vec<f32> = common::get_var_float_value(&file, "TEMP".to_string(), temp_fill_value, vec_size)?;
    let temp_qc: Vec<String> = common::get_char_value(&file, "TEMP_QC".to_string(), vec_size)?;

    // PSAL
    let psal_fill_value: f32 = common::get_float_fill_value(&file, "PSAL".to_string());
    let psal: Vec<f32> = common::get_var_float_value(&file, "PSAL".to_string(), psal_fill_value, vec_size)?;
    let psal_qc: Vec<String> = common::get_char_value(&file, "PSAL_QC".to_string(), vec_size)?;

    // PRES
    let pres_fill_value: f32 = common::get_float_fill_value(&file, "PRES".to_string());
    let pres: Vec<f32> = common::get_var_float_value(&file, "PRES".to_string(), pres_fill_value, vec_size)?;
    let pres_qc: Vec<String> = common::get_char_value(&file, "PRES_QC".to_string(), vec_size)?;

    // Create a DataFrame using the Polars crate
    let timestamp_series = Series::new("profile_timestamp".into(), timestamp);
    let df = DataFrame::new(vec![
        Series::new("platform_code".into(), platform_code),
        Series::new("profile_no".into(), profile_no),
        Series::new("profile_time".into(), time),
        timestamp_series.cast(&DataType::Datetime(TimeUnit::Milliseconds, None))?,
        Series::new("observation_no".into(), obs_no),
        Series::new("longitude".into(), longitude),
        Series::new("latitude".into(), latitude),
        Series::new("filename".into(), filename),
        Series::new("temp".into(), temp),
        Series::new("temp_qc".into(), temp_qc),
        Series::new("psal".into(), psal),
        Series::new("psal_qc".into(), psal_qc),
        Series::new("pres".into(), pres),
        Series::new("pres_qc".into(), pres_qc),
    ])?;

    Ok(df)
}

pub fn run(args: &[String]) -> Result<Config, Box<dyn Error>> {
    let config = ConvertConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    match netcdf_to_parquet(&config) {
        Ok(_config) => {
            Ok(Config {
                module: "convert".to_string(),
                target: "cora_legacy".to_string(),
                args: args.to_vec(),
            })
        }
        Err(e) => {
            eprintln!("Conversion failed: {}", e);
            process::exit(1);
        }
    }
}
