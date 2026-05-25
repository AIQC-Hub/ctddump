use std::error::Error;
use std::path::Path;

use num_traits::{Float, FromPrimitive};
use chrono::{Duration, TimeZone, Utc};
use netcdf;
use netcdf::{NcTypeDescriptor, types::NcVariableType};
use gsw::conversions::{p_from_z, z_from_p};
use crate::convert::common;

#[repr(transparent)]
#[derive(Copy, Clone)]
struct NcChar(i8);

unsafe impl NcTypeDescriptor for NcChar {
    fn type_descriptor() -> NcVariableType {
        NcVariableType::Char
    }
}

pub fn get_string_value(attr_value: netcdf::AttributeValue) -> Option<String> {
    match attr_value {
        netcdf::AttributeValue::Str(s) => Some(s),
        _ => None, // None for non-string variants
    }
}

pub fn get_base_file_name(file_name: &str) -> Result<String, Box<dyn Error>> {
    let path = Path::new(file_name);
    let base_name = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_else(|| {
        panic!("Cannot get base file name");
    });

    Ok(base_name.to_string())
}

pub fn create_platform_code(
    file: &netcdf::File,
    vec_size: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let attr = file.attribute("platform_code").unwrap();
    let attr_val= common::get_string_value(attr.value().unwrap()).unwrap();

    let profile_vec: Vec<String> = vec![attr_val.clone(); vec_size];

    Ok(profile_vec)
}

pub fn create_profile_no_sequence(time_len: usize, obs_len: usize) -> Result<Vec<u32>, Box<dyn Error>> {
    let profile_no: Vec<u32> = (1..=time_len as u32).collect();
    let profile_no_repeated: Vec<u32> = profile_no.iter().flat_map(|&v| std::iter::repeat(v).take(obs_len)).collect();

    Ok(profile_no_repeated)
}

pub fn create_observation_sequence(time_len: usize, obs_len: usize) -> Result<Vec<u32>, Box<dyn Error>> {
    let obs: Vec<u32> = (1..=obs_len as u32).collect();
    let obs_repeated: Vec<u32> = obs.iter().cycle().take(obs_len * time_len).cloned().collect();

    Ok(obs_repeated)
}

pub fn convert_time_value(time_values: &Vec<f64>) -> Result<Vec<i64>, Box<dyn Error>> {
    let reference_date = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0)
        .single() // Get the single result
        .ok_or("Failed to create reference date")?;
    let seconds_in_day = 86400.0; // 24 * 60 * 60

    let timestamps: Vec<i64> = time_values
        .iter()
        .map(|&time_value| {
            let whole_days = time_value.trunc() as i64;
            let fractional_day = time_value - whole_days as f64;

            let duration = Duration::days(whole_days);
            let fractional_duration = Duration::seconds((fractional_day * seconds_in_day) as i64);
            let date = reference_date + duration + fractional_duration;

            date.timestamp_millis()
        })
        .collect();

    Ok(timestamps)
}

pub fn get_coordinate_value<T>(
    file: &netcdf::File,
    var_name: String,
    time_len: usize,
    obs_len: usize,
    fill_value: T,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Clone + Copy + netcdf::NcTypeDescriptor,
{

    let var = match file.variable(&var_name) {
        Some(v) => v,
        None => {
            return Ok(std::iter::repeat(fill_value)
                .take(time_len * obs_len)
                .collect())
        }
    };

    let var_value: Vec<T> = var.get::<T, _>(..).unwrap().iter().cloned().collect();
    let var_value_repeated: Vec<T> = if var_value.len() == 1 {
        let value = var_value[0];
        std::iter::repeat(value).take(time_len * obs_len).collect()
    } else {
        // Repeat each value `depth_len` times
        var_value.iter().flat_map(|&v| std::iter::repeat(v).take(obs_len)).collect()
    };

    Ok(var_value_repeated)
}

pub fn get_float_fill_value (
    file: &netcdf::File,
    var_name: String
) -> f32 {

    let var = match file.variable(&var_name) {
        Some(v) => v,
        None => return f32::NAN,
    };

    let fill_value = match var.attribute_value("_FillValue") {
        Some(v) => v,
        _ => return f32::NAN,
    };

    match fill_value.unwrap() {
        netcdf::AttributeValue::Float(v) => v,
        _ => panic!("Unsupported type: {}.", var_name),
    }
}

pub fn get_var_float_value<T>(
    file: &netcdf::File,
    var_name: String,
    fill_value: T,
    vec_size: usize,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Float + FromPrimitive + netcdf::NcTypeDescriptor,
{
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => {
            return Ok(vec![T::nan(); vec_size]);
        }
    };
    let var_val_temp: Vec<T> = var_temp.get::<T, _>(..).unwrap().iter().cloned().collect();

    let var_val_clean: Vec<T> = if var_val_temp.len() == 1 {
        let value = var_val_temp[0];
        std::iter::repeat(value).take(vec_size).collect()
    } else {
        // Repeat each value `depth_len` times
        var_val_temp.into_iter().map(|x| if x == fill_value { T::nan() } else { x }).collect()
    };

    Ok(var_val_clean)
}

/// Convert a raw i8 QC byte (0–9) to its single-digit string.
/// Any value outside 0–9 (including `i8::MIN` used as "missing") maps to `""`.
fn i8_to_qc_string(v: i8) -> String {
    if (0..=9).contains(&v) {
        char::from(b'0' + v as u8).to_string()
    } else {
        String::new()
    }
}

/// Read an integer QC variable (stored as `i8`) and return each flag as a
/// single-character string ("0"–"9"). Missing or out-of-range values → `""`.
pub fn get_qc_value(
    file: &netcdf::File,
    var_name: String,
    vec_size: usize,
) -> Result<Vec<String>, Box<dyn Error>>
{
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => return Ok(vec![String::new(); vec_size]),
    };

    let raw: Vec<i8> = var_temp.get::<i8, _>(..)?.iter().cloned().collect();
    Ok(raw.iter().map(|&v| i8_to_qc_string(v)).collect())
}

/// Read a coordinate-tiled QC variable (e.g., `TIME_QC`, `POSITION_QC`) stored
/// as `i8` and return each flag as a single-character string.
/// The variable is tiled from `time_len` to `time_len × obs_len` like other
/// coordinates. Missing variable → all `""`.
pub fn get_qc_coordinate_value(
    file: &netcdf::File,
    var_name: String,
    time_len: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>>
{
    let raw: Vec<i8> = get_coordinate_value(file, var_name, time_len, obs_len, i8::MIN)?;
    Ok(raw.iter().map(|&v| i8_to_qc_string(v)).collect())
}

pub fn get_char_value (
    file: &netcdf::File,
    var_name: String,
    vec_size: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => {
            return Ok(vec![" ".to_string(); vec_size]);
        }
    };

    let char_data: Vec<NcChar> = var_temp.get_values::<NcChar, _>(..)?;
    let vec_strings: Vec<String> = char_data
        .iter()
        .map(|&NcChar(c)| (c as u8 as char).to_string())
        .collect();

    Ok(vec_strings)
}

pub fn get_char_value2 (
    file: &netcdf::File,
    var_name: String,
    nrow: usize,
    ncol: usize,
    max_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => {
            return Ok(vec![" ".to_string(); nrow * ncol]);
        }
    };

    let char_data: Vec<NcChar> = var_temp.get_values::<NcChar, _>(..)?;

    let mut result = Vec::with_capacity(ncol);
    for col in 0..ncol {
        // For each row, collect characters from columns, filtering out whitespace and null chars
        let row_string: String = (0..max_len)
            .filter_map(|row| {
                let idx = col * max_len + row;
                let char_val = char_data[idx].0 as u8 as char;
                if char_val != '\0' && char_val != ' ' {
                    Some(char_val)
                } else {
                    None
                }
            })
            .collect();

        // Add the row string to the result vector
        result.push(row_string);
    }

    let result_repeated: Vec<String> = result
        .iter()
        .flat_map(|v| std::iter::repeat(v.clone()).take(nrow))
        .collect();

    Ok(result_repeated)
}

pub fn get_char_vector3(
    file: &netcdf::File,
    var_name: String,
    nrow: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => {
            return Ok(vec![]);
        }
    };

    let char_data: Vec<NcChar> = var_temp.get_values::<NcChar, _>(..)?;
    let char_vec: Vec<String> = char_data.iter().map(|nc| (nc.0 as u8 as char).to_string()).collect();

    // Repeat each character as a String nrow times
    let result: Vec<String> = char_vec.iter().flat_map(|s| std::iter::repeat(s.clone()).take(nrow)).collect();

    Ok(result)
}

/// Reads a NetCDF char QC variable and returns each character as a `String`.
/// Space (`' '`) and null (`'\0'`) are treated as missing and map to `""`.
/// All other characters (digits `'0'`–`'9'`, letters `'A'`, `'B'`, …) are
/// kept as-is so that non-numeric ARGO QC codes are preserved faithfully.
pub fn get_qc_value_from_char(
    file: &netcdf::File,
    var_name: String,
    vec_size: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let char_vals = get_char_value(file, var_name, vec_size)?;
    let result: Vec<String> = char_vals
        .iter()
        .map(|s| match s.chars().next() {
            Some(c) if c != '\0' && c != ' ' => c.to_string(),
            _ => String::new(),
        })
        .collect();
    Ok(result)
}

pub fn convert_depth_to_pressure<T>(
    pres: Vec<f32>,
    pres_qc: Vec<String>,
    deph: Vec<f32>,
    deph_qc: Vec<String>,
    fill_value: f32,
    latitude: Vec<T>,
) -> (Vec<f32>, Vec<String>, Vec<i8>)
where
    T: Into<f64> + Copy,
{
    let n = deph.len().min(pres.len()).min(latitude.len());
    let mut converted_pres = pres.clone();
    let mut converted_pres_qc = pres_qc.clone();
    let mut pres_conv = vec![0_i8; pres.len()];  // Initialize pres_conv with zeros

    for i in 0..n {
        if (pres[i].is_nan() || (pres[i] == fill_value)) && !deph[i].is_nan() {
            let depth_in_meters = -(deph[i] as f64);
            match p_from_z(depth_in_meters, latitude[i].into(), None, None) {
                Ok(pressure_value) => {
                    converted_pres[i] = pressure_value as f32;
                    converted_pres_qc[i] = deph_qc[i].clone();
                    pres_conv[i] = 1;  // Mark conversion status
                }
                Err(_) => {
                    converted_pres[i] = f32::NAN;
                    // pres_conv[i] remains 0, as initialized
                }
            }
        }
    }

    (converted_pres, converted_pres_qc, pres_conv)
}

pub fn convert_pressure_to_depth<T>(
    deph: Vec<f32>,
    deph_qc: Vec<String>,
    pres: Vec<f32>,
    pres_qc: Vec<String>,
    fill_value: f32,
    latitude: Vec<T>,
) -> (Vec<f32>, Vec<String>, Vec<i8>)
where
    T: Into<f64> + Copy,
{
    let n = deph.len().min(pres.len()).min(latitude.len());
    let mut converted_deph = deph.clone();
    let mut converted_deph_qc = deph_qc.clone();
    let mut deph_conv = vec![0_i8; pres.len()]; // 0 = not converted, 1 = converted

    for i in 0..n {
        if !pres[i].is_nan() && (deph[i].is_nan() || (deph[i] == fill_value)) {
            // call gsw::conversions::z_from_p with four f64 arguments
            let z_value = z_from_p(
                pres[i] as f64,
                latitude[i].into(),
                0.0_f64,  // geo_strf_dyn_height (typically 0 if unknown)
                0.0_f64,  // sea_surface_geopotential (typically 0 if unknown)
            );

            converted_deph[i] = z_value as f32 * -1.0_f32;
            converted_deph_qc[i] = pres_qc[i].clone();
            deph_conv[i] = 1;
        }
    }

    (converted_deph, converted_deph_qc, deph_conv)
}

/// Expand a deployment-indexed coordinate variable (`DEPLOY_LATITUDE` or
/// `DEPLOY_LONGITUDE`) into a flat `Vec<f32>` of length `time_len × obs_len`.
///
/// The `DEPLOYMENT` variable holds the 0-based TIME index at which each
/// deployment begins. For each TIME step `t`, the active deployment is the
/// one with the latest start index that is still ≤ `t`. Each TIME value is
/// then repeated `obs_len` times to match the observation-level output.
pub fn expand_deploy_coords(
    file: &netcdf::File,
    var_name: &str,
    time_len: usize,
    obs_len: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let vec_size = time_len * obs_len;

    let deploy_var = match file.variable("DEPLOYMENT") {
        Some(v) => v,
        None => return Ok(vec![f32::NAN; vec_size]),
    };
    let deploy_indices: Vec<i32> = deploy_var.get::<i32, _>(..)?.iter().cloned().collect();

    let coord_var = match file.variable(var_name) {
        Some(v) => v,
        None => return Ok(vec![f32::NAN; vec_size]),
    };
    let coord_values: Vec<f32> = coord_var.get::<f32, _>(..)?.iter().cloned().collect();

    if deploy_indices.len() != coord_values.len() {
        return Err(format!(
            "DEPLOYMENT and {} have different lengths ({} vs {})",
            var_name,
            deploy_indices.len(),
            coord_values.len()
        )
        .into());
    }

    // Sort deployments by their start TIME index (ascending)
    let mut sorted: Vec<(usize, f32)> = deploy_indices
        .iter()
        .zip(coord_values.iter())
        .map(|(&idx, &val)| (idx.max(0) as usize, val))
        .collect();
    sorted.sort_by_key(|&(idx, _)| idx);

    let mut result = Vec::with_capacity(vec_size);
    for t in 0..time_len {
        // Latest deployment whose start index ≤ t
        let value = sorted
            .iter()
            .rev()
            .find(|&&(start, _)| start <= t)
            .map(|&(_, val)| val)
            .unwrap_or(f32::NAN);
        for _ in 0..obs_len {
            result.push(value);
        }
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── i8_to_qc_string ──────────────────────────────────────────────────────

    #[test]
    fn test_i8_to_qc_string_all_digits() {
        for d in 0i8..=9 {
            assert_eq!(i8_to_qc_string(d), d.to_string(), "digit {d}");
        }
    }

    #[test]
    fn test_i8_to_qc_string_min_is_empty() {
        assert_eq!(i8_to_qc_string(i8::MIN), "");
    }

    #[test]
    fn test_i8_to_qc_string_negative_is_empty() {
        assert_eq!(i8_to_qc_string(-1), "");
    }

    #[test]
    fn test_i8_to_qc_string_above_9_is_empty() {
        assert_eq!(i8_to_qc_string(10), "");
        assert_eq!(i8_to_qc_string(100), "");
    }

    // ── convert_time_value ───────────────────────────────────────────────────

    #[test]
    fn test_convert_time_value_epoch() {
        // days = 0 → the reference date 1950-01-01 00:00:00 UTC
        let result = convert_time_value(&vec![0.0]).unwrap();
        let expected = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0).unwrap().timestamp_millis();
        assert_eq!(result, vec![expected]);
    }

    #[test]
    fn test_convert_time_value_half_day() {
        // 0.5 days → noon on 1950-01-01
        let result = convert_time_value(&vec![0.5]).unwrap();
        let expected = Utc.with_ymd_and_hms(1950, 1, 1, 12, 0, 0).unwrap().timestamp_millis();
        assert_eq!(result, vec![expected]);
    }

    #[test]
    fn test_convert_time_value_known_date() {
        // compute days from 1950-01-01 to 2000-01-01 via chrono (avoids hardcoding)
        let epoch  = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0).unwrap();
        let target = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let days = (target - epoch).num_days() as f64;

        let result = convert_time_value(&vec![days]).unwrap();
        assert_eq!(result, vec![target.timestamp_millis()]);
    }

    #[test]
    fn test_convert_time_value_consecutive_days() {
        let epoch_ms = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0).unwrap().timestamp_millis();
        let result = convert_time_value(&vec![0.0, 1.0]).unwrap();
        assert_eq!(result[0], epoch_ms);
        assert_eq!(result[1], epoch_ms + 86_400_000); // +1 day in ms
    }

    #[test]
    fn test_convert_time_value_empty() {
        assert!(convert_time_value(&vec![]).unwrap().is_empty());
    }

    // ── get_base_file_name ───────────────────────────────────────────────────

    #[test]
    fn test_get_base_file_name_simple() {
        assert_eq!(get_base_file_name("file.nc").unwrap(), "file");
    }

    #[test]
    fn test_get_base_file_name_relative_path() {
        assert_eq!(
            get_base_file_name("./data/AR_PR_CT_ITP-71.nc").unwrap(),
            "AR_PR_CT_ITP-71"
        );
    }

    #[test]
    fn test_get_base_file_name_absolute_path() {
        assert_eq!(
            get_base_file_name("/ocean/CO_DMQCGL01_19861204_PR_CT.nc").unwrap(),
            "CO_DMQCGL01_19861204_PR_CT"
        );
    }

    // ── create_profile_no_sequence ───────────────────────────────────────────

    #[test]
    fn test_profile_no_sequence_basic() {
        // 3 profiles × 2 obs → [1,1, 2,2, 3,3]
        assert_eq!(create_profile_no_sequence(3, 2).unwrap(), vec![1u32, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn test_profile_no_sequence_single_profile() {
        assert_eq!(create_profile_no_sequence(1, 4).unwrap(), vec![1u32, 1, 1, 1]);
    }

    #[test]
    fn test_profile_no_sequence_single_obs() {
        assert_eq!(create_profile_no_sequence(3, 1).unwrap(), vec![1u32, 2, 3]);
    }

    // ── create_observation_sequence ──────────────────────────────────────────

    #[test]
    fn test_observation_sequence_basic() {
        // 2 profiles × 3 obs → [1,2,3, 1,2,3]
        assert_eq!(create_observation_sequence(2, 3).unwrap(), vec![1u32, 2, 3, 1, 2, 3]);
    }

    #[test]
    fn test_observation_sequence_single_obs() {
        assert_eq!(create_observation_sequence(3, 1).unwrap(), vec![1u32, 1, 1]);
    }

    #[test]
    fn test_observation_sequence_restarts_each_profile() {
        // obs numbers always restart at 1 for each new profile
        let seq = create_observation_sequence(4, 5).unwrap();
        assert_eq!(seq.len(), 20);
        for p in 0..4usize {
            assert_eq!(seq[p * 5],     1, "profile {p} obs start");
            assert_eq!(seq[p * 5 + 4], 5, "profile {p} obs end");
        }
    }

    // ── convert_depth_to_pressure ────────────────────────────────────────────

    #[test]
    fn test_d2p_converts_nan_pressure() {
        // pres=NaN, deph=100 m at 45°N → compute and mark pres_conv=1
        let (cp, cpq, pc) = convert_depth_to_pressure(
            vec![f32::NAN], vec!["".to_string()],
            vec![100.0_f32], vec!["1".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert!(!cp[0].is_nan(), "pressure should be computed");
        assert!(cp[0] > 0.0);
        assert_eq!(cpq[0], "1"); // QC copied from deph_qc
        assert_eq!(pc[0], 1);
    }

    #[test]
    fn test_d2p_keeps_existing_pressure() {
        // pres already set → unchanged, pres_conv=0
        let (cp, cpq, pc) = convert_depth_to_pressure(
            vec![101.5_f32], vec!["2".to_string()],
            vec![100.0_f32], vec!["1".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert_eq!(cp[0], 101.5);
        assert_eq!(cpq[0], "2");
        assert_eq!(pc[0], 0);
    }

    #[test]
    fn test_d2p_skips_nan_depth() {
        // pres=NaN, deph=NaN → no conversion
        let (cp, _, pc) = convert_depth_to_pressure(
            vec![f32::NAN], vec!["".to_string()],
            vec![f32::NAN], vec!["".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert!(cp[0].is_nan());
        assert_eq!(pc[0], 0);
    }

    #[test]
    fn test_d2p_qc_copied_from_deph() {
        let (_, cpq, _) = convert_depth_to_pressure(
            vec![f32::NAN], vec!["".to_string()],
            vec![50.0_f32], vec!["4".to_string()],
            f32::NAN, vec![60.0_f64],
        );
        assert_eq!(cpq[0], "4");
    }

    #[test]
    fn test_d2p_multiple_entries() {
        // entry 0: convert (pres NaN, deph valid)
        // entry 1: keep (pres valid)
        // entry 2: skip (both NaN)
        let (cp, _, pc) = convert_depth_to_pressure(
            vec![f32::NAN,   200.0_f32, f32::NAN],
            vec!["".to_string(), "1".to_string(), "".to_string()],
            vec![100.0_f32, 100.0_f32, f32::NAN],
            vec!["1".to_string(), "1".to_string(), "".to_string()],
            f32::NAN, vec![45.0_f64, 45.0, 45.0],
        );
        assert!(!cp[0].is_nan());
        assert_eq!(cp[1], 200.0);
        assert!(cp[2].is_nan());
        assert_eq!(pc, vec![1, 0, 0]);
    }

    // ── convert_pressure_to_depth ────────────────────────────────────────────

    #[test]
    fn test_p2d_converts_nan_depth() {
        // deph=NaN, pres=100 dbar at 45°N → compute and mark deph_conv=1
        let (cd, cdq, dc) = convert_pressure_to_depth(
            vec![f32::NAN], vec!["".to_string()],
            vec![100.0_f32], vec!["1".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert!(!cd[0].is_nan(), "depth should be computed");
        assert!(cd[0] > 0.0);
        assert_eq!(cdq[0], "1"); // QC copied from pres_qc
        assert_eq!(dc[0], 1);
    }

    #[test]
    fn test_p2d_keeps_existing_depth() {
        // deph already set → unchanged, deph_conv=0
        let (cd, cdq, dc) = convert_pressure_to_depth(
            vec![99.0_f32], vec!["2".to_string()],
            vec![100.0_f32], vec!["1".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert_eq!(cd[0], 99.0);
        assert_eq!(cdq[0], "2");
        assert_eq!(dc[0], 0);
    }

    #[test]
    fn test_p2d_skips_nan_pressure() {
        // pres=NaN → no conversion
        let (cd, _, dc) = convert_pressure_to_depth(
            vec![f32::NAN], vec!["".to_string()],
            vec![f32::NAN], vec!["".to_string()],
            f32::NAN, vec![45.0_f64],
        );
        assert!(cd[0].is_nan());
        assert_eq!(dc[0], 0);
    }

    // ── roundtrips ───────────────────────────────────────────────────────────

    #[test]
    fn test_depth_pressure_roundtrip() {
        // depth → pressure → depth should recover within 0.5 m at various depths/latitudes
        for &(depth, lat) in &[(50.0_f32, 0.0_f64), (200.0, 45.0), (1000.0, 80.0)] {
            let (cp, _, _) = convert_depth_to_pressure(
                vec![f32::NAN], vec!["".to_string()],
                vec![depth], vec!["1".to_string()],
                f32::NAN, vec![lat],
            );
            let (cd, _, _) = convert_pressure_to_depth(
                vec![f32::NAN], vec!["".to_string()],
                vec![cp[0]], vec!["1".to_string()],
                f32::NAN, vec![lat],
            );
            let err = (cd[0] - depth).abs();
            assert!(err < 0.5, "depth={depth} lat={lat}: roundtrip error {err}");
        }
    }

    #[test]
    fn test_pressure_depth_roundtrip() {
        // pressure → depth → pressure should recover within 0.5 dbar
        for &(pres, lat) in &[(50.0_f32, 0.0_f64), (200.0, 45.0), (1000.0, 80.0)] {
            let (cd, _, _) = convert_pressure_to_depth(
                vec![f32::NAN], vec!["".to_string()],
                vec![pres], vec!["1".to_string()],
                f32::NAN, vec![lat],
            );
            let (cp, _, _) = convert_depth_to_pressure(
                vec![f32::NAN], vec!["".to_string()],
                vec![cd[0]], vec!["1".to_string()],
                f32::NAN, vec![lat],
            );
            let err = (cp[0] - pres).abs();
            assert!(err < 0.5, "pres={pres} lat={lat}: roundtrip error {err}");
        }
    }
}
