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
    create_profile_no_sequence_chunk(0, time_len, obs_len)
}

/// Profile numbers (1-based, global) for the TIME slice `[time_offset, time_offset + time_count)`,
/// each repeated `obs_len` times. Streaming-safe: numbering stays consistent across chunks
/// because it is anchored to the absolute `time_offset`, not the chunk position.
pub fn create_profile_no_sequence_chunk(
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<u32>, Box<dyn Error>> {
    let profile_no: Vec<u32> = ((time_offset as u32 + 1)..=(time_offset + time_count) as u32).collect();
    Ok(profile_no.iter().flat_map(|&v| std::iter::repeat(v).take(obs_len)).collect())
}

/// Target number of observation rows to assemble per streamed chunk. Bounds peak
/// memory independent of file size; override with `CTDDUMP_CHUNK_ROWS` for tuning
/// (larger = faster/more memory) or tests. Values ≤ 0 / unparsable fall back to the default.
pub fn chunk_rows() -> usize {
    const DEFAULT_CHUNK_ROWS: usize = 1_000_000;
    std::env::var("CTDDUMP_CHUNK_ROWS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_CHUNK_ROWS)
}

/// Split the outer dimension (`TIME` / `N_PROF`, length `outer_len`) into
/// `(offset, count)` chunks so each chunk holds at most ~`chunk_rows()` observation
/// rows (`count × obs_len`), always advancing by ≥ 1 outer step. Returns an empty
/// vec when `outer_len == 0`.
pub fn time_chunks(outer_len: usize, obs_len: usize) -> Vec<(usize, usize)> {
    let step = (chunk_rows() / obs_len.max(1)).max(1);
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < outer_len {
        let count = step.min(outer_len - offset);
        chunks.push((offset, count));
        offset += count;
    }
    chunks
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

/// Read the TIME slice `[time_offset, time_offset + time_count)` of a 1-D coordinate
/// variable and tile each value `obs_len` times. A scalar variable (length 1) is
/// broadcast across the whole chunk; a missing variable yields `fill_value`.
pub fn get_coordinate_value_chunk<T>(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
    fill_value: T,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Clone + Copy + netcdf::NcTypeDescriptor,
{
    let n = time_count * obs_len;
    let var = match file.variable(var_name) {
        Some(v) => v,
        None => return Ok(vec![fill_value; n]),
    };

    // Scalar coordinate: one value tiled across the whole chunk. Read with the
    // full-extent selector so it works whether the variable is rank-0 or a
    // length-1 dimension (an explicit `[0..1]` range fails on a rank-0 scalar).
    if var.len() == 1 {
        let value = var.get_values::<T, _>(..)?[0];
        return Ok(vec![value; n]);
    }

    // 1-D TIME-indexed coordinate: read only this chunk's slice, tile each `obs_len`.
    let slice: Vec<T> = var.get_values::<T, _>([time_offset..time_offset + time_count])?;
    Ok(slice.iter().flat_map(|&v| std::iter::repeat(v).take(obs_len)).collect())
}

pub fn get_float_fill_value (
    file: &netcdf::File,
    var_name: &str
) -> f32 {

    let var = match file.variable(var_name) {
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

/// Read the TIME slice `[time_offset, time_offset + time_count)` of a 2-D
/// `[TIME, DEPTH]` float variable, replacing `fill_value` with NaN. A scalar
/// variable (length 1) is broadcast; a missing variable yields all-NaN.
pub fn get_var_float_value_chunk<T>(
    file: &netcdf::File,
    var_name: &str,
    fill_value: T,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Float + FromPrimitive + netcdf::NcTypeDescriptor,
{
    let n = time_count * obs_len;
    let var_temp = match file.variable(var_name) {
        Some(var) => var,
        None => return Ok(vec![T::nan(); n]),
    };

    // Scalar: broadcast the single value (matches the original whole-file behaviour,
    // which did not apply the fill→NaN mapping in the scalar case). Full-extent read
    // so it works for rank-0 scalars as well as length-1 dimensions.
    if var_temp.len() == 1 {
        let value = var_temp.get_values::<T, _>(..)?[0];
        return Ok(vec![value; n]);
    }

    let slice: Vec<T> =
        var_temp.get_values::<T, _>([time_offset..time_offset + time_count, 0..obs_len])?;
    Ok(slice.into_iter().map(|x| if x == fill_value { T::nan() } else { x }).collect())
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

/// Read the slice `[time_offset, time_offset + time_count)` of a 2-D `[TIME, DEPTH]`
/// integer QC variable (`i8`) and return each flag as a single-character string
/// ("0"–"9"). Missing variable or out-of-range values → `""`.
pub fn get_qc_value_chunk(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>>
{
    let n = time_count * obs_len;
    let var_temp = match file.variable(var_name) {
        Some(var) => var,
        None => return Ok(vec![String::new(); n]),
    };

    let raw: Vec<i8> = var_temp.get_values::<i8, _>([time_offset..time_offset + time_count, 0..obs_len])?;
    Ok(raw.iter().map(|&v| i8_to_qc_string(v)).collect())
}

/// Read a coordinate-tiled QC variable (e.g., `TIME_QC`, `POSITION_QC`) stored
/// as `i8` for the TIME slice `[time_offset, time_offset + time_count)` and return
/// each flag as a single-character string. Tiled from TIME to `time_count × obs_len`
/// like other coordinates. Missing variable → all `""`.
pub fn get_qc_coordinate_value_chunk(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>>
{
    let raw: Vec<i8> = get_coordinate_value_chunk(file, var_name, time_offset, time_count, obs_len, i8::MIN)?;
    Ok(raw.iter().map(|&v| i8_to_qc_string(v)).collect())
}

/// Read the slice `[time_offset, time_offset + time_count)` of a 2-D `[TIME, DEPTH]`
/// `char` variable, one character per observation. Missing variable → all `" "`.
pub fn get_char_value_chunk(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let n = time_count * obs_len;
    let var_temp = match file.variable(var_name) {
        Some(var) => var,
        None => return Ok(vec![" ".to_string(); n]),
    };

    let char_data: Vec<NcChar> =
        var_temp.get_values::<NcChar, _>([time_offset..time_offset + time_count, 0..obs_len])?;
    Ok(char_data.iter().map(|&NcChar(c)| (c as u8 as char).to_string()).collect())
}

/// Read the profile slice `[prof_offset, prof_offset + prof_count)` of a 2-D
/// `[N_PROF, STRING<max_len>]` `char` variable (e.g. `PLATFORM_NUMBER`), assembling
/// one trimmed string per profile and repeating it `obs_len` (`N_LEVELS`) times.
/// Missing variable → all `" "`.
pub fn get_char_value2_chunk(
    file: &netcdf::File,
    var_name: &str,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
    max_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let var_temp = match file.variable(var_name) {
        Some(var) => var,
        None => return Ok(vec![" ".to_string(); prof_count * obs_len]),
    };

    let char_data: Vec<NcChar> =
        var_temp.get_values::<NcChar, _>([prof_offset..prof_offset + prof_count, 0..max_len])?;

    let mut result = Vec::with_capacity(prof_count);
    for col in 0..prof_count {
        // Collect the characters of this profile's name, dropping whitespace and null chars
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
        result.push(row_string);
    }

    Ok(result
        .iter()
        .flat_map(|v| std::iter::repeat(v.clone()).take(obs_len))
        .collect())
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
pub fn get_qc_value_from_char_chunk(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let char_vals = get_char_value_chunk(file, var_name, time_offset, time_count, obs_len)?;
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

/// Pure algorithm for expanding deployment-indexed coordinates.
///
/// Given parallel slices of deployment start indices and coordinate values,
/// builds a flat `Vec<f32>` of length `time_len × obs_len`.  For each TIME
/// step `t` the active deployment is the one with the latest start index that
/// is still ≤ `t`; if no deployment covers `t` yet the value is `NaN`.  Each
/// TIME value is repeated `obs_len` times to match the observation-level output.
///
/// Negative indices are clamped to 0.  Input need not be sorted.
/// Whole-file convenience wrapper over [`expand_coords_from_indices_range`],
/// retained for the unit tests that exercise the expansion algorithm directly.
#[cfg(test)]
fn expand_coords_from_indices(
    deploy_indices: &[i32],
    coord_values: &[f32],
    time_len: usize,
    obs_len: usize,
) -> Vec<f32> {
    expand_coords_from_indices_range(deploy_indices, coord_values, 0, time_len, obs_len)
}

/// Streaming variant of [`expand_coords_from_indices`] restricted to the TIME range
/// `[t_start, t_end)`. Deployment resolution is anchored to absolute TIME indices,
/// so a chunk resolves to exactly the same values it would in a whole-file pass.
fn expand_coords_from_indices_range(
    deploy_indices: &[i32],
    coord_values: &[f32],
    t_start: usize,
    t_end: usize,
    obs_len: usize,
) -> Vec<f32> {
    // Sort deployments by their start TIME index (ascending)
    let mut sorted: Vec<(usize, f32)> = deploy_indices
        .iter()
        .zip(coord_values.iter())
        .map(|(&idx, &val)| (idx.max(0) as usize, val))
        .collect();
    sorted.sort_by_key(|&(idx, _)| idx);

    let mut result = Vec::with_capacity((t_end - t_start) * obs_len);
    for t in t_start..t_end {
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

    result
}

/// Expand a deployment-indexed coordinate variable (`DEPLOY_LATITUDE` or
/// `DEPLOY_LONGITUDE`) into a flat `Vec<f32>` for the TIME slice
/// `[time_offset, time_offset + time_count)`.
///
/// The `DEPLOYMENT` variable holds the 0-based TIME index at which each
/// deployment begins. For each TIME step `t`, the active deployment is the
/// one with the latest start index that is still ≤ `t`. Each TIME value is
/// then repeated `obs_len` times to match the observation-level output.
///
/// `DEPLOYMENT` / `DEPLOY_*` are per-deployment (few entries), so they are read
/// in full every chunk — the resolution is anchored to absolute TIME indices,
/// making each chunk's result identical to a whole-file pass.
pub fn expand_deploy_coords_chunk(
    file: &netcdf::File,
    var_name: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let n = time_count * obs_len;

    let deploy_var = match file.variable("DEPLOYMENT") {
        Some(v) => v,
        None => return Ok(vec![f32::NAN; n]),
    };
    let deploy_indices: Vec<i32> = deploy_var.get_values::<i32, _>(..)?;

    let coord_var = match file.variable(var_name) {
        Some(v) => v,
        None => return Ok(vec![f32::NAN; n]),
    };
    let coord_values: Vec<f32> = coord_var.get_values::<f32, _>(..)?;

    if deploy_indices.len() != coord_values.len() {
        return Err(format!(
            "DEPLOYMENT and {} have different lengths ({} vs {})",
            var_name,
            deploy_indices.len(),
            coord_values.len()
        )
        .into());
    }

    Ok(expand_coords_from_indices_range(
        &deploy_indices,
        &coord_values,
        time_offset,
        time_offset + time_count,
        obs_len,
    ))
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

    // ── expand_coords_from_indices ───────────────────────────────────────────

    #[test]
    fn test_expand_single_deployment_covers_all_times() {
        // One deployment starts at t=0 → same coordinate for every time step
        let result = expand_coords_from_indices(&[0], &[10.5_f32], 4, 1);
        assert_eq!(result, vec![10.5, 10.5, 10.5, 10.5]);
    }

    #[test]
    fn test_expand_two_deployments_boundary() {
        // Deployment 0 starts at t=0 (lon=1.0), deployment 1 starts at t=3 (lon=2.0)
        // obs_len=1 for simplicity: result length = 5
        let result = expand_coords_from_indices(&[0, 3], &[1.0_f32, 2.0], 5, 1);
        assert_eq!(result, vec![1.0, 1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn test_expand_obs_tiling() {
        // With obs_len=3 each time-step value must repeat 3 times
        // One deployment at t=0 (lon=5.0), 2 time steps → [5.0, 5.0, 5.0, 5.0, 5.0, 5.0]
        let result = expand_coords_from_indices(&[0], &[5.0_f32], 2, 3);
        assert_eq!(result, vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0]);
    }

    #[test]
    fn test_expand_obs_tiling_two_deployments() {
        // Deployment 0 at t=0 (lon=1.0), deployment 1 at t=2 (lon=3.0), obs_len=2, time_len=4
        // t=0 → [1.0,1.0], t=1 → [1.0,1.0], t=2 → [3.0,3.0], t=3 → [3.0,3.0]
        let result = expand_coords_from_indices(&[0, 2], &[1.0_f32, 3.0], 4, 2);
        assert_eq!(result, vec![1.0, 1.0, 1.0, 1.0, 3.0, 3.0, 3.0, 3.0]);
    }

    #[test]
    fn test_expand_unsorted_input_sorted_correctly() {
        // Supply indices in reverse order — algorithm must sort before resolving
        // deployment B starts at t=0 (lon=20.0), deployment A starts at t=3 (lon=10.0)
        // → t=0..2 use 20.0, t=3..4 use 10.0
        let result = expand_coords_from_indices(&[3, 0], &[10.0_f32, 20.0], 5, 1);
        assert_eq!(result, vec![20.0, 20.0, 20.0, 10.0, 10.0]);
    }

    #[test]
    fn test_expand_deployment_starts_after_t0_nan_early() {
        // First deployment starts at t=2 → t=0 and t=1 have no active deployment → NaN
        let result = expand_coords_from_indices(&[2], &[7.0_f32], 4, 1);
        assert!(result[0].is_nan(), "t=0 should be NaN");
        assert!(result[1].is_nan(), "t=1 should be NaN");
        assert_eq!(result[2], 7.0);
        assert_eq!(result[3], 7.0);
    }

    #[test]
    fn test_expand_three_deployments() {
        // Deployments: t=0 → 1.0, t=2 → 2.0, t=4 → 3.0, time_len=6, obs_len=1
        let result = expand_coords_from_indices(&[0, 2, 4], &[1.0_f32, 2.0, 3.0], 6, 1);
        assert_eq!(result, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    }

    #[test]
    fn test_expand_negative_index_clamped_to_zero() {
        // A negative index is clamped to 0 and behaves like a deployment at t=0
        let result = expand_coords_from_indices(&[-5], &[99.0_f32], 3, 1);
        assert_eq!(result, vec![99.0, 99.0, 99.0]);
    }

    #[test]
    fn test_expand_result_length() {
        // Result must always equal time_len × obs_len
        let result = expand_coords_from_indices(&[0, 3], &[1.0_f32, 2.0], 5, 4);
        assert_eq!(result.len(), 20);
    }
}
