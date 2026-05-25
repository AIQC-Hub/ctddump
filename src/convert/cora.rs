use std::process;
use std::error::Error;

use netcdf;
use polars::prelude::*;

use crate::Config;
use super::ConvertConfig;
use super::common;
use super::cora_config::{CoraConfig, QcType};

fn netcdf_to_parquet(
    convert_config: &ConvertConfig,
    cora_config: &CoraConfig,
) -> Result<(), Box<dyn Error>> {
    let df = collect_cora_data(&convert_config.src_file, cora_config)?;

    let mask = df.column("temp")?.is_not_nan()?
        | df.column("psal")?.is_not_nan()?
        | df.column("pres")?.is_not_nan()?;
    let mut filtered = df.filter(&mask)?;

    let mut out_file = std::fs::File::create(&convert_config.target_file)?;
    ParquetWriter::new(&mut out_file).finish(&mut filtered)?;

    Ok(())
}

fn collect_cora_data(
    src_file: &str,
    config: &CoraConfig,
) -> Result<DataFrame, Box<dyn Error>> {
    let file = netcdf::open(src_file)?;

    let n_profs = file.dimension("N_PROF").unwrap().len();
    let n_levels = file.dimension("N_LEVELS").unwrap().len();
    let string32 = file.dimension("STRING32").unwrap().len();
    let vec_size = n_profs * n_levels;

    // Keys
    let platform_code: Vec<String> = common::get_char_value2(&file, "PLATFORM_NUMBER".to_string(), n_levels, n_profs, string32)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence(n_profs, n_levels)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(n_profs, n_levels)?;

    // Time (variable name from config)
    let time: Vec<f64> = common::get_coordinate_value(&file, config.time_var.clone(), n_profs, n_levels, f64::NAN)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;

    let longitude: Vec<f64> = common::get_coordinate_value(&file, "LONGITUDE".to_string(), n_profs, n_levels, f64::NAN)?;
    let latitude: Vec<f64> = common::get_coordinate_value(&file, "LATITUDE".to_string(), n_profs, n_levels, f64::NAN)?;

    // TIME_QC / POSITION_QC: only read when the source format has them as i8.
    // Legacy files may store these as char or omit them; fill with "" in that case.
    let time_qc: Vec<String> = if config.has_time_qc {
        common::get_qc_coordinate_value(&file, "TIME_QC".to_string(), n_profs, n_levels)?
    } else {
        vec!["".to_string(); vec_size]
    };
    let position_qc: Vec<String> = if config.has_time_qc {
        common::get_qc_coordinate_value(&file, "POSITION_QC".to_string(), n_profs, n_levels)?
    } else {
        vec!["".to_string(); vec_size]
    };

    let basename = common::get_base_file_name(src_file)?;
    let filename: Vec<String> = vec![basename; vec_size];

    // TEMP
    let temp_fill = common::get_float_fill_value(&file, "TEMP".to_string());
    let temp: Vec<f32> = common::get_var_float_value(&file, "TEMP".to_string(), temp_fill, vec_size)?;
    let temp_qc: Vec<String> = read_qc(&file, "TEMP_QC", vec_size, &config.qc_type)?;

    // PSAL
    let psal_fill = common::get_float_fill_value(&file, "PSAL".to_string());
    let psal: Vec<f32> = common::get_var_float_value(&file, "PSAL".to_string(), psal_fill, vec_size)?;
    let psal_qc: Vec<String> = read_qc(&file, "PSAL_QC", vec_size, &config.qc_type)?;

    // PRES
    let pres_fill = common::get_float_fill_value(&file, "PRES".to_string());
    let pres: Vec<f32> = common::get_var_float_value(&file, "PRES".to_string(), pres_fill, vec_size)?;
    let pres_qc: Vec<String> = read_qc(&file, "PRES_QC", vec_size, &config.qc_type)?;

    // DEPH: bidirectional conversion if source has DEPH; otherwise fill with NaN
    let (converted_pres, converted_pres_qc, pres_conv, converted_deph, converted_deph_qc, deph_conv) =
        if config.has_deph_source {
            let deph_fill = common::get_float_fill_value(&file, "DEPH".to_string());
            let deph: Vec<f32> = common::get_var_float_value(&file, "DEPH".to_string(), deph_fill, vec_size)?;
            let deph_qc: Vec<String> = read_qc(&file, "DEPH_QC", vec_size, &config.qc_type)?;
            let (cp, cpq, pc) = common::convert_depth_to_pressure(pres.clone(), pres_qc.clone(), deph.clone(), deph_qc.clone(), pres_fill, latitude.clone());
            let (cd, cdq, dc) = common::convert_pressure_to_depth(deph.clone(), deph_qc.clone(), pres.clone(), pres_qc.clone(), deph_fill, latitude.clone());
            (cp, cpq, pc, cd, cdq, dc)
        } else {
            let deph = vec![f32::NAN; vec_size];
            let deph_qc: Vec<String> = vec!["".to_string(); vec_size];
            let pres_conv = vec![0_i8; vec_size];
            let deph_conv = vec![0_i8; vec_size];
            (pres.clone(), pres_qc.clone(), pres_conv, deph, deph_qc, deph_conv)
        };

    // CORA has no profile-specific coordinate source; add NaN columns so the
    // output schema matches the NRT schema (profile_longitude / profile_latitude).
    let profile_longitude: Vec<f64> = vec![f64::NAN; vec_size];
    let profile_latitude: Vec<f64> = vec![f64::NAN; vec_size];

    let timestamp_series = Series::new("profile_timestamp".into(), timestamp);
    let df = DataFrame::new(vec![
        Series::new("platform_code".into(), platform_code),
        Series::new("profile_no".into(), profile_no),
        Series::new("profile_time".into(), time),
        timestamp_series.cast(&DataType::Datetime(TimeUnit::Milliseconds, None))?,
        Series::new("observation_no".into(), obs_no),
        Series::new("longitude".into(), longitude),
        Series::new("latitude".into(), latitude),
        Series::new("profile_longitude".into(), profile_longitude),
        Series::new("profile_latitude".into(), profile_latitude),
        Series::new("time_qc".into(), time_qc),
        Series::new("position_qc".into(), position_qc),
        Series::new("filename".into(), filename),
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

/// Read a QC variable as a `String` per flag, converting from char if needed.
fn read_qc(
    file: &netcdf::File,
    var_name: &str,
    vec_size: usize,
    qc_type: &QcType,
) -> Result<Vec<String>, Box<dyn Error>> {
    match qc_type {
        QcType::Int => common::get_qc_value(file, var_name.to_string(), vec_size),
        QcType::Char => common::get_qc_value_from_char(file, var_name.to_string(), vec_size),
    }
}

/// Convert a single NetCDF file to Parquet. Called by both `run` and the batch processor.
pub fn convert_file(src: &str, dest: &str, config: &CoraConfig) -> Result<(), Box<dyn Error>> {
    let convert_config = ConvertConfig {
        src_file: src.to_string(),
        target_file: dest.to_string(),
    };
    netcdf_to_parquet(&convert_config, config)
}

pub fn run(args: &[String], cora_config: CoraConfig, target: &str) -> Result<Config, Box<dyn Error>> {
    let config = ConvertConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    match netcdf_to_parquet(&config, &cora_config) {
        Ok(_) => Ok(Config {
            module: "convert".to_string(),
            target: target.to_string(),
            args: args.to_vec(),
        }),
        Err(e) => {
            eprintln!("Conversion failed: {}", e);
            process::exit(1);
        }
    }
}
