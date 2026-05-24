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

    // Extract gl data
    let df_gl_data = collect_netcdf_nrt_gl_data(src_file)?;

    // Filter NAN rows
    let mask = df_gl_data.column("temp")?.is_not_nan()?
        | df_gl_data.column("psal")?.is_not_nan()?
        | df_gl_data.column("pres")?.is_not_nan()?
        | df_gl_data.column("deph")?.is_not_nan()?;
    let mut filtered_df = df_gl_data.filter(&mask)?;

    // Write Parguet file
    let mut out_file = std::fs::File::create(target_file).unwrap();
    ParquetWriter::new(&mut out_file).finish(&mut filtered_df).unwrap();

    Ok(())
}

fn collect_netcdf_nrt_gl_data(
    src_file: &String,
) -> Result<DataFrame, Box<dyn std::error::Error>> {

    // Open the NetCDF file
    let file = netcdf::open(src_file)?;

    // Dimensions
    let time_len = file.dimension("TIME").unwrap().len();
    let obs_len = file.dimension("DEPTH").unwrap().len();
    let vec_size = time_len * obs_len;

    // Keys
    let platform_code: Vec<String> = common::create_platform_code(&file, vec_size)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence(time_len, obs_len)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(time_len, obs_len)?;

    // Meta info
    let time: Vec<f64> = common::get_coordinate_value(&file, "TIME".to_string(), time_len, obs_len, f64::NAN)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;
    let longitude: Vec<f32> = common::get_coordinate_value(&file, "LONGITUDE".to_string(), time_len, obs_len, f32::NAN)?;
    let latitude: Vec<f32> = common::get_coordinate_value(&file, "LATITUDE".to_string(), time_len, obs_len, f32::NAN)?;
    let time_qc: Vec<i8> = common::get_coordinate_value(&file, "TIME_QC".to_string(), time_len, obs_len, i8::MIN)?;
    let position_qc: Vec<i8> = common::get_coordinate_value(&file, "POSITION_QC".to_string(), time_len, obs_len, i8::MIN)?;
    let basename = common::get_base_file_name(&src_file)?;
    let filename: Vec<String> = vec![basename.clone(); vec_size];

    // Temp
    let temp_fill_value: f32 = common::get_float_fill_value(&file, "TEMP".to_string());
    let temp: Vec<f32> = common::get_var_float_value(&file, "TEMP".to_string(), temp_fill_value, vec_size)?;
    let temp_qc: Vec<i8> = common::get_qc_value(&file, "TEMP_QC".to_string(), vec_size)?;

    // PSAL
    let psal_fill_value: f32 = common::get_float_fill_value(&file, "PSAL".to_string());
    let psal: Vec<f32> = common::get_var_float_value(&file, "PSAL".to_string(), psal_fill_value, vec_size)?;
    let psal_qc: Vec<i8> = common::get_qc_value(&file, "PSAL_QC".to_string(), vec_size)?;

    // PRES
    let pres_fill_value: f32 = common::get_float_fill_value(&file, "PRES".to_string());
    let pres: Vec<f32> = common::get_var_float_value(&file, "PRES".to_string(), pres_fill_value, vec_size)?;
    let pres_qc: Vec<i8> = common::get_qc_value(&file, "PRES_QC".to_string(), vec_size)?;

    // DEPH
    let deph_fill_value: f32 = common::get_float_fill_value(&file, "DEPH".to_string());
    let deph: Vec<f32> = common::get_var_float_value(&file, "DEPH".to_string(), deph_fill_value, vec_size)?;
    let deph_qc: Vec<i8> = common::get_qc_value(&file, "DEPH_QC".to_string(), vec_size)?;

    // Convert DEPH to PRES when PRES is NA
    let (converted_pres, converted_pres_qc, pres_conv) = common::convert_depth_to_pressure(pres.clone(), pres_qc.clone(), deph.clone(), deph_qc.clone(), pres_fill_value, latitude.clone());

    // Convert PRES to DEPH when DEPH is NA
    let (converted_deph, converted_deph_qc, deph_conv) = common::convert_pressure_to_depth(deph.clone(), deph_qc.clone(), pres.clone(), pres_qc.clone(), deph_fill_value, latitude.clone());

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
        Series::new("time_qc".into(), time_qc),
        Series::new("position_qc".into(), position_qc),
        Series::new("temp".into(), temp),
        Series::new("temp_qc".into(), temp_qc),
        Series::new("psal".into(), psal),
        Series::new("psal_qc".into(), psal_qc),
        Series::new("pres".into(), converted_pres),
        Series::new("pres_qc".into(), converted_pres_qc),
        Series::new("pres_conv".into(), pres_conv),
        Series::new("deph".into(), converted_deph),
        Series::new("deph_qc".into(), converted_deph_qc),
        Series::new("deph_conv".into(), deph_conv),
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
                module: "netcdf".to_string(),
                target: "nrt_gl".to_string(),
                args: args.to_vec(),
            })
        }
        Err(e) => {
            eprintln!("Conversion failed: {}", e);
            process::exit(1);
        }
    }
}
