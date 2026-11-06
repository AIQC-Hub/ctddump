use std::error::Error;

use netcdf;
use polars::prelude::*;

use crate::Config;
use super::ConvertConfig;
use super::common;
use super::cora_config::CoraConfig;
use super::nrt::dimension_len;
use super::source::SourceSpec;

fn netcdf_to_parquet(
    convert_config: &ConvertConfig,
    cora_config: &CoraConfig,
) -> Result<(), Box<dyn Error>> {
    let file = netcdf::open(&convert_config.src_file)?;
    let basename = common::get_base_file_name(&convert_config.src_file)?;
    let spec = SourceSpec::cora(cora_config);
    write_parquet(&file, &basename, &convert_config.target_file, cora_config.has_deph_source, &spec)
}

/// Convert an open profile-level (`[profile, level]`, one platform per profile)
/// NetCDF file to Parquet, reading every name from `spec`. Shared by the CORA
/// formats and the `generic` format.
pub(crate) fn write_parquet(
    file: &netcdf::File,
    basename: &str,
    target_file: &str,
    has_deph_source: bool,
    spec: &SourceSpec,
) -> Result<(), Box<dyn Error>> {
    let n_profs = dimension_len(file, &spec.profile_dim)?;
    let n_levels = dimension_len(file, &spec.level_dim)?;

    let out_file = std::fs::File::create(target_file)?;

    // Stream the file in N_PROF chunks, writing each as a Parquet row group so the
    // full dense N_PROF × N_LEVELS grid is never materialized at once. An empty
    // (zero-row) chunk defines the schema, guaranteeing it matches the real chunks.
    // `collect_cora_chunk` already drops all-NaN rows (on the raw vectors, not via
    // Polars' leaky `filter`), and `set_parallel(false)` avoids the parquet writer's
    // parallel-encoding path, which leaks memory per call in this Polars version —
    // both leaks accumulate without bound across a large batch of files.
    let empty = collect_cora_chunk(file, spec, has_deph_source, basename, 0, 0, n_levels)?;
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;

    let chunks = common::time_chunks(n_profs, n_levels);
    if chunks.is_empty() {
        writer.write_batch(&empty)?;
    }
    for (p0, pc) in chunks {
        let df = collect_cora_chunk(file, spec, has_deph_source, basename, p0, pc, n_levels)?;
        writer.write_batch(&df)?;
    }
    writer.finish()?;

    Ok(())
}

fn collect_cora_chunk(
    file: &netcdf::File,
    spec: &SourceSpec,
    has_deph_source: bool,
    basename: &str,
    prof_offset: usize,
    prof_count: usize,
    n_levels: usize,
) -> Result<DataFrame, Box<dyn Error>> {
    let vec_size = prof_count * n_levels;

    // Keys
    let platform_code: Vec<String> = common::platform_codes_chunk(file, spec, prof_offset, prof_count, n_levels)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence_chunk(prof_offset, prof_count, n_levels)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(prof_count, n_levels)?;

    // Time (variable name from config)
    let time: Vec<f64> = common::get_time_chunk(file, spec, prof_offset, prof_count, n_levels)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;

    let longitude: Vec<f64> = common::get_coord_chunk(file, &spec.longitude, spec.defaults.longitude, spec, prof_offset, prof_count, n_levels)?;
    let latitude: Vec<f64> = common::get_coord_chunk(file, &spec.latitude, spec.defaults.latitude, spec, prof_offset, prof_count, n_levels)?;

    // TIME_QC / POSITION_QC: `None` in the spec when the source format lacks them
    // (CORA legacy stores them as char under other names); filled with "" then.
    let qc_type = spec.qc_type.as_ref();
    let time_qc: Vec<String> = common::read_qc_coord_chunk(file, spec.time_qc.as_deref(), qc_type, prof_offset, prof_count, n_levels)?;
    let position_qc: Vec<String> = common::read_qc_coord_chunk(file, spec.position_qc.as_deref(), qc_type, prof_offset, prof_count, n_levels)?;

    let filename: Vec<String> = vec![basename.to_string(); vec_size];

    // Data mode: CORA has no per-parameter `<PARAM>_DM` variables, only the
    // per-profile `DATA_MODE[N_PROF]`, which applies to every parameter of the
    // profile. Missing variable -> "R"; fill cells -> "".
    let data_mode: Vec<String> = common::get_dm_coordinate_value_chunk(file, &spec.data_mode, spec.dm_fallback(), prof_offset, prof_count, n_levels)?;

    // TEMP
    let (temp, _) = common::get_param_chunk(file, &spec.temp.name, spec, prof_offset, prof_count, n_levels)?;
    let temp_qc: Vec<String> = common::read_qc_chunk(file, &spec.temp.qc, qc_type, prof_offset, prof_count, n_levels)?;

    // PSAL
    let (psal, _) = common::get_param_chunk(file, &spec.psal.name, spec, prof_offset, prof_count, n_levels)?;
    let psal_qc: Vec<String> = common::read_qc_chunk(file, &spec.psal.qc, qc_type, prof_offset, prof_count, n_levels)?;

    // PRES
    let (pres, pres_fill) = common::get_param_chunk(file, &spec.pres.name, spec, prof_offset, prof_count, n_levels)?;
    let pres_qc: Vec<String> = common::read_qc_chunk(file, &spec.pres.qc, qc_type, prof_offset, prof_count, n_levels)?;

    // DEPH: bidirectional conversion if source has DEPH; otherwise fill with NaN
    let (converted_pres, converted_pres_qc, pres_conv, converted_deph, converted_deph_qc, deph_conv) =
        if has_deph_source {
            let (deph, deph_fill) = common::get_param_chunk(file, &spec.deph.name, spec, prof_offset, prof_count, n_levels)?;
            let deph_qc: Vec<String> = common::read_qc_chunk(file, &spec.deph.qc, qc_type, prof_offset, prof_count, n_levels)?;
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

    // Drop rows where temp, psal and pres are all missing — on the raw vectors,
    // before building the DataFrame, to avoid Polars' leaky row-gather (see
    // common::retain_by_mask).
    let keep: Vec<bool> = (0..vec_size)
        .map(|i| !temp[i].is_nan() || !psal[i].is_nan() || !converted_pres[i].is_nan())
        .collect();

    let platform_code = common::retain_by_mask(platform_code, &keep);
    let profile_no = common::retain_by_mask(profile_no, &keep);
    let time = common::retain_by_mask(time, &keep);
    let timestamp = common::retain_by_mask(timestamp, &keep);
    let obs_no = common::retain_by_mask(obs_no, &keep);
    let longitude = common::retain_by_mask(longitude, &keep);
    let latitude = common::retain_by_mask(latitude, &keep);
    let profile_longitude = common::retain_by_mask(profile_longitude, &keep);
    let profile_latitude = common::retain_by_mask(profile_latitude, &keep);
    let time_qc = common::retain_by_mask(time_qc, &keep);
    let position_qc = common::retain_by_mask(position_qc, &keep);
    let filename = common::retain_by_mask(filename, &keep);
    let temp = common::retain_by_mask(temp, &keep);
    let temp_qc = common::retain_by_mask(temp_qc, &keep);
    let data_mode = common::retain_by_mask(data_mode, &keep);
    let psal = common::retain_by_mask(psal, &keep);
    let psal_qc = common::retain_by_mask(psal_qc, &keep);
    let converted_pres = common::retain_by_mask(converted_pres, &keep);
    let converted_pres_qc = common::retain_by_mask(converted_pres_qc, &keep);
    let pres_conv = common::retain_by_mask(pres_conv, &keep);
    let converted_deph = common::retain_by_mask(converted_deph, &keep);
    let converted_deph_qc = common::retain_by_mask(converted_deph_qc, &keep);
    let deph_conv = common::retain_by_mask(deph_conv, &keep);

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
        Series::new("temp_dm".into(), data_mode.clone()),
        Series::new("psal".into(), psal),
        Series::new("psal_qc".into(), psal_qc),
        Series::new("psal_dm".into(), data_mode.clone()),
        Series::new("pres".into(), converted_pres),
        Series::new("pres_qc".into(), converted_pres_qc),
        Series::new("pres_dm".into(), data_mode),
        Series::new("pres_conv".into(), pres_conv),
        Series::new("deph".into(), converted_deph),
        Series::new("deph_qc".into(), converted_deph_qc),
        Series::new("deph_conv".into(), deph_conv),
    ])?;

    Ok(df)
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
    let config = ConvertConfig::build(args)?;
    netcdf_to_parquet(&config, &cora_config)?;
    Ok(Config {
        module: "convert".to_string(),
        target: target.to_string(),
        args: args.to_vec(),
    })
}
