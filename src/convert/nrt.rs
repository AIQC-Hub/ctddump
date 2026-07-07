use std::process;
use std::error::Error;

use netcdf;
use polars::prelude::*;

use crate::Config;
use super::ConvertConfig;
use super::common;
use super::nrt_config::NrtConfig;

fn netcdf_to_parquet(
    convert_config: &ConvertConfig,
    nrt_config: &NrtConfig,
) -> Result<(), Box<dyn Error>> {
    let file = netcdf::open(&convert_config.src_file)?;
    let time_len = file.dimension("TIME").unwrap().len();
    let obs_len = file.dimension("DEPTH").unwrap().len();
    let basename = common::get_base_file_name(&convert_config.src_file)?;

    // Use DEPH whenever the file actually contains it, even if the region default
    // (`has_deph_source`) is `false`. Some AR/MO files ship DEPH instead of PRES;
    // ignoring it would leave both `pres` and `deph` empty. When DEPH is absent the
    // sourced/derived branches are equivalent, so this never changes PRES-only output.
    let has_deph = nrt_config.has_deph_source || file.variable("DEPH").is_some();

    let out_file = std::fs::File::create(&convert_config.target_file)?;

    // Stream the file in TIME chunks, writing each chunk as a Parquet row group so
    // the full dense TIME × DEPTH grid is never materialized at once. An empty
    // (zero-row) chunk defines the schema, guaranteeing it matches the real chunks.
    // `collect_nrt_chunk` already drops all-NaN rows (on the raw vectors, not via
    // Polars' leaky `filter`), and `set_parallel(false)` avoids the parquet writer's
    // parallel-encoding path, which leaks memory per call in this Polars version —
    // both leaks accumulate without bound across a large batch of files.
    let empty = collect_nrt_chunk(&file, nrt_config, has_deph, &basename, 0, 0, obs_len)?;
    let schema = empty.schema();
    let mut writer = ParquetWriter::new(out_file).set_parallel(false).batched(&schema)?;

    let chunks = common::time_chunks(time_len, obs_len);
    if chunks.is_empty() {
        // No TIME steps: still emit a valid, empty Parquet file with the schema.
        writer.write_batch(&empty)?;
    }
    for (t0, tc) in chunks {
        let df = collect_nrt_chunk(&file, nrt_config, has_deph, &basename, t0, tc, obs_len)?;
        writer.write_batch(&df)?;
    }
    writer.finish()?;

    Ok(())
}

/// Resolve profile-level coordinates from the best available source:
///   1. PRECISE_LONGITUDE / PRECISE_LATITUDE (preferred)
///   2. DEPLOY_LONGITUDE / DEPLOY_LATITUDE expanded via the DEPLOYMENT index
///   3. NaN — if neither source exists
///
/// Returns (profile_longitude, profile_latitude), each of length time_count × obs_len,
/// for the TIME slice `[time_offset, time_offset + time_count)`.
fn get_profile_coords_chunk(
    file: &netcdf::File,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
    let n = time_count * obs_len;

    if file.variable("PRECISE_LONGITUDE").is_some() {
        let lon = common::get_coordinate_value_chunk(file, "PRECISE_LONGITUDE", time_offset, time_count, obs_len, f32::NAN)?;
        let lat = common::get_coordinate_value_chunk(file, "PRECISE_LATITUDE", time_offset, time_count, obs_len, f32::NAN)?;
        return Ok((lon, lat));
    }

    if file.variable("DEPLOY_LONGITUDE").is_some() {
        let lon = common::expand_deploy_coords_chunk(file, "DEPLOY_LONGITUDE", time_offset, time_count, obs_len)?;
        let lat = common::expand_deploy_coords_chunk(file, "DEPLOY_LATITUDE", time_offset, time_count, obs_len)?;
        return Ok((lon, lat));
    }

    Ok((vec![f32::NAN; n], vec![f32::NAN; n]))
}

/// Fill NaN values in `primary` from `fallback`.
fn fill_nan_from(primary: &[f32], fallback: &[f32]) -> Vec<f32> {
    primary
        .iter()
        .zip(fallback.iter())
        .map(|(&p, &f)| if p.is_nan() { f } else { p })
        .collect()
}

fn collect_nrt_chunk(
    file: &netcdf::File,
    config: &NrtConfig,
    has_deph: bool,
    basename: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<DataFrame, Box<dyn Error>> {
    let vec_size = time_count * obs_len;

    // Keys
    let platform_code: Vec<String> = common::create_platform_code(file, vec_size)?;
    let profile_no: Vec<u32> = common::create_profile_no_sequence_chunk(time_offset, time_count, obs_len)?;
    let obs_no: Vec<u32> = common::create_observation_sequence(time_count, obs_len)?;

    // Time
    let time: Vec<f64> = common::get_coordinate_value_chunk(file, "TIME", time_offset, time_count, obs_len, f64::NAN)?;
    let timestamp: Vec<i64> = common::convert_time_value(&time)?;

    // Standard coordinates (always read)
    let longitude_raw: Vec<f32> = common::get_coordinate_value_chunk(file, "LONGITUDE", time_offset, time_count, obs_len, f32::NAN)?;
    let latitude_raw: Vec<f32> = common::get_coordinate_value_chunk(file, "LATITUDE", time_offset, time_count, obs_len, f32::NAN)?;

    // Profile-level coordinates (PRECISE_* preferred over DEPLOY_*).
    // Cross-fill is only applied when profile coords are enabled; otherwise
    // profile columns are left as NaN so the absence of a profile source is clear.
    let (longitude, latitude, profile_longitude, profile_latitude) = if config.has_profile_coords {
        let (plon_raw, plat_raw) = get_profile_coords_chunk(file, time_offset, time_count, obs_len)?;
        let lon  = fill_nan_from(&longitude_raw, &plon_raw);
        let lat  = fill_nan_from(&latitude_raw,  &plat_raw);
        let plon = fill_nan_from(&plon_raw, &longitude_raw);
        let plat = fill_nan_from(&plat_raw, &latitude_raw);
        (lon, lat, plon, plat)
    } else {
        (longitude_raw, latitude_raw, vec![f32::NAN; vec_size], vec![f32::NAN; vec_size])
    };

    let time_qc: Vec<String> = common::get_qc_coordinate_value_chunk(file, "TIME_QC", time_offset, time_count, obs_len)?;
    let position_qc: Vec<String> = common::get_qc_coordinate_value_chunk(file, "POSITION_QC", time_offset, time_count, obs_len)?;

    let filename: Vec<String> = vec![basename.to_string(); vec_size];

    // TEMP
    let temp_fill = common::get_float_fill_value(file, "TEMP");
    let temp: Vec<f32> = common::get_var_float_value_chunk(file, "TEMP", temp_fill, time_offset, time_count, obs_len)?;
    let temp_qc: Vec<String> = common::get_qc_value_chunk(file, "TEMP_QC", time_offset, time_count, obs_len)?;

    // PSAL
    let psal_fill = common::get_float_fill_value(file, "PSAL");
    let psal: Vec<f32> = common::get_var_float_value_chunk(file, "PSAL", psal_fill, time_offset, time_count, obs_len)?;
    let psal_qc: Vec<String> = common::get_qc_value_chunk(file, "PSAL_QC", time_offset, time_count, obs_len)?;

    // PRES
    let pres_fill = common::get_float_fill_value(file, "PRES");
    let pres: Vec<f32> = common::get_var_float_value_chunk(file, "PRES", pres_fill, time_offset, time_count, obs_len)?;
    let pres_qc: Vec<String> = common::get_qc_value_chunk(file, "PRES_QC", time_offset, time_count, obs_len)?;

    // PRES / DEPH conversion.
    // When profile coords are enabled, use profile_latitude (already cross-filled
    // from latitude where NaN) for better accuracy. When disabled, profile_latitude
    // is all-NaN, so fall back to the standard latitude to avoid NaN conversions.
    let conversion_latitude = if config.has_profile_coords {
        profile_latitude.clone()
    } else {
        latitude.clone()
    };

    let (converted_pres, converted_pres_qc, pres_conv, converted_deph, converted_deph_qc, deph_conv) =
        if has_deph {
            let deph_fill = common::get_float_fill_value(file, "DEPH");
            let deph: Vec<f32> = common::get_var_float_value_chunk(file, "DEPH", deph_fill, time_offset, time_count, obs_len)?;
            let deph_qc: Vec<String> = common::get_qc_value_chunk(file, "DEPH_QC", time_offset, time_count, obs_len)?;
            let (cp, cpq, pc) = common::convert_depth_to_pressure(pres.clone(), pres_qc.clone(), deph.clone(), deph_qc.clone(), pres_fill, conversion_latitude.clone());
            let (cd, cdq, dc) = common::convert_pressure_to_depth(deph.clone(), deph_qc.clone(), pres.clone(), pres_qc.clone(), deph_fill, conversion_latitude.clone());
            (cp, cpq, pc, cd, cdq, dc)
        } else {
            let deph = vec![pres_fill; vec_size];
            let deph_qc: Vec<String> = vec!["".to_string(); vec_size];
            let pres_conv = vec![0_i8; vec_size];
            let (cd, cdq, dc) = common::convert_pressure_to_depth(deph.clone(), deph_qc.clone(), pres.clone(), pres_qc.clone(), pres_fill, conversion_latitude.clone());
            (pres.clone(), pres_qc.clone(), pres_conv, cd, cdq, dc)
        };

    // Drop rows where temp, psal and pres (plus deph, when sourced) are all missing
    // — on the raw vectors, before building the DataFrame, to avoid Polars' leaky
    // row-gather (see common::retain_by_mask).
    let keep: Vec<bool> = (0..vec_size)
        .map(|i| {
            !temp[i].is_nan()
                || !psal[i].is_nan()
                || !converted_pres[i].is_nan()
                || (has_deph && !converted_deph[i].is_nan())
        })
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

/// Convert a single NetCDF file to Parquet. Called by both `run` and the batch processor.
pub fn convert_file(src: &str, dest: &str, config: &NrtConfig) -> Result<(), Box<dyn Error>> {
    let convert_config = ConvertConfig {
        src_file: src.to_string(),
        target_file: dest.to_string(),
    };
    netcdf_to_parquet(&convert_config, config)
}

pub fn run(args: &[String], nrt_config: NrtConfig, target: &str) -> Result<Config, Box<dyn Error>> {
    let config = ConvertConfig::build(args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1);
    });

    match netcdf_to_parquet(&config, &nrt_config) {
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

#[cfg(test)]
mod tests {
    use super::fill_nan_from;

    #[test]
    fn test_fill_nan_from_replaces_nans_with_fallback() {
        let primary  = vec![1.0_f32, f32::NAN, 3.0];
        let fallback = vec![10.0_f32, 20.0,    30.0];
        assert_eq!(fill_nan_from(&primary, &fallback), vec![1.0, 20.0, 3.0]);
    }

    #[test]
    fn test_fill_nan_from_no_nans_unchanged() {
        let primary  = vec![1.0_f32, 2.0, 3.0];
        let fallback = vec![10.0_f32, 20.0, 30.0];
        assert_eq!(fill_nan_from(&primary, &fallback), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_fill_nan_from_all_nans_uses_fallback() {
        let primary  = vec![f32::NAN, f32::NAN];
        let fallback = vec![10.0_f32, 20.0];
        assert_eq!(fill_nan_from(&primary, &fallback), vec![10.0, 20.0]);
    }

    #[test]
    fn test_fill_nan_from_nan_fallback_stays_nan() {
        // if both are NaN, result is NaN
        let primary  = vec![f32::NAN];
        let fallback = vec![f32::NAN];
        assert!(fill_nan_from(&primary, &fallback)[0].is_nan());
    }

    #[test]
    fn test_fill_nan_from_preserves_zeros() {
        // 0.0 is not NaN; it must not be replaced
        let primary  = vec![0.0_f32, f32::NAN];
        let fallback = vec![99.0_f32, 99.0];
        assert_eq!(fill_nan_from(&primary, &fallback), vec![0.0, 99.0]);
    }
}
