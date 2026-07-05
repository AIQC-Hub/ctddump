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
    let file = netcdf::open(&convert_config.src_file)?;
    let n_profs = file.dimension("N_PROF").unwrap().len();
    let n_levels = file.dimension("N_LEVELS").unwrap().len();
    let string32 = file.dimension("STRING32").unwrap().len();
    let basename = common::get_base_file_name(&convert_config.src_file)?;

    let out_file = std::fs::File::create(&convert_config.target_file)?;

    // Stream the file in N_PROF chunks, writing each as a Parquet row group so the
    // full dense N_PROF × N_LEVELS grid is never materialized at once. An empty
    // (zero-row) chunk defines the schema, guaranteeing it matches the real chunks.
    let empty = collect_cora_chunk(&file, cora_config, &basename, 0, 0, n_levels, string32)?;
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).batched(&schema)?;

    let chunks = common::time_chunks(n_profs, n_levels);
    if chunks.is_empty() {
        writer.write_batch(&empty)?;
    }
    for (p0, pc) in chunks {
        let df = collect_cora_chunk(&file, cora_config, &basename, p0, pc, n_levels, string32)?;

        let mask = df.column("temp")?.is_not_nan()?
            | df.column("psal")?.is_not_nan()?
            | df.column("pres")?.is_not_nan()?;
        let filtered = df.filter(&mask)?;
        writer.write_batch(&filtered)?;
    }
    writer.finish()?;

    Ok(())
}

fn collect_cora_chunk(
    file: &netcdf::File,
    config: &CoraConfig,
    basename: &str,
    prof_offset: usize,
    prof_count: usize,
    n_levels: usize,
    string32: usize,
) -> Result<DataFrame, Box<dyn Error>> {
    let vec_size = prof_count * n_levels;

    // Keys
    let platform_code: Vec<String> = common::get_char_value2_chunk(file, "PLATFORM_NUMBER", prof_offset, prof_count, n_levels, string32)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence_chunk(prof_offset, prof_count, n_levels)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(prof_count, n_levels)?;

    // Time (variable name from config)
    let time: Vec<f64> = common::get_coordinate_value_chunk(file, &config.time_var, prof_offset, prof_count, n_levels, f64::NAN)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;

    let longitude: Vec<f64> = common::get_coordinate_value_chunk(file, "LONGITUDE", prof_offset, prof_count, n_levels, f64::NAN)?;
    let latitude: Vec<f64> = common::get_coordinate_value_chunk(file, "LATITUDE", prof_offset, prof_count, n_levels, f64::NAN)?;

    // TIME_QC / POSITION_QC: only read when the source format has them as i8.
    // Legacy files may store these as char or omit them; fill with "" in that case.
    let time_qc: Vec<String> = if config.has_time_qc {
        common::get_qc_coordinate_value_chunk(file, "TIME_QC", prof_offset, prof_count, n_levels)?
    } else {
        vec!["".to_string(); vec_size]
    };
    let position_qc: Vec<String> = if config.has_time_qc {
        common::get_qc_coordinate_value_chunk(file, "POSITION_QC", prof_offset, prof_count, n_levels)?
    } else {
        vec!["".to_string(); vec_size]
    };

    let filename: Vec<String> = vec![basename.to_string(); vec_size];

    // TEMP
    let temp_fill = common::get_float_fill_value(file, "TEMP");
    let temp: Vec<f32> = common::get_var_float_value_chunk(file, "TEMP", temp_fill, prof_offset, prof_count, n_levels)?;
    let temp_qc: Vec<String> = read_qc_chunk(file, "TEMP_QC", &config.qc_type, prof_offset, prof_count, n_levels)?;

    // PSAL
    let psal_fill = common::get_float_fill_value(file, "PSAL");
    let psal: Vec<f32> = common::get_var_float_value_chunk(file, "PSAL", psal_fill, prof_offset, prof_count, n_levels)?;
    let psal_qc: Vec<String> = read_qc_chunk(file, "PSAL_QC", &config.qc_type, prof_offset, prof_count, n_levels)?;

    // PRES
    let pres_fill = common::get_float_fill_value(file, "PRES");
    let pres: Vec<f32> = common::get_var_float_value_chunk(file, "PRES", pres_fill, prof_offset, prof_count, n_levels)?;
    let pres_qc: Vec<String> = read_qc_chunk(file, "PRES_QC", &config.qc_type, prof_offset, prof_count, n_levels)?;

    // DEPH: bidirectional conversion if source has DEPH; otherwise fill with NaN
    let (converted_pres, converted_pres_qc, pres_conv, converted_deph, converted_deph_qc, deph_conv) =
        if config.has_deph_source {
            let deph_fill = common::get_float_fill_value(file, "DEPH");
            let deph: Vec<f32> = common::get_var_float_value_chunk(file, "DEPH", deph_fill, prof_offset, prof_count, n_levels)?;
            let deph_qc: Vec<String> = read_qc_chunk(file, "DEPH_QC", &config.qc_type, prof_offset, prof_count, n_levels)?;
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

/// Read a QC variable for the profile slice `[prof_offset, prof_offset + prof_count)`
/// as a `String` per flag, converting from char if the source format stores it that way.
fn read_qc_chunk(
    file: &netcdf::File,
    var_name: &str,
    qc_type: &QcType,
    prof_offset: usize,
    prof_count: usize,
    n_levels: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    match qc_type {
        QcType::Int => common::get_qc_value_chunk(file, var_name, prof_offset, prof_count, n_levels),
        QcType::Char => common::get_qc_value_from_char_chunk(file, var_name, prof_offset, prof_count, n_levels),
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
