use std::error::Error;
use std::path::Path;

use num_traits::{Bounded, Float, FromPrimitive};
use chrono::{Duration, TimeZone, Utc};
use netcdf;
use netcdf::{NcTypeDescriptor, types::NcVariableType};
use gsw::conversions::{p_from_z, z_from_p};
use crate::netcdf::common;

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

pub fn get_qc_value<T>(
    file: &netcdf::File,
    var_name: String,
    vec_size: usize,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Bounded + Clone + Copy + netcdf::NcTypeDescriptor,
{
    let var_temp = match file.variable(&var_name) {
        Some(var) => var,
        None => {
            return Ok(vec![T::min_value(); vec_size]);
        }
    };

    let qc_value: Vec<T> = var_temp.get::<T, _>(..)?.iter().cloned().collect();

    Ok(qc_value)
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

pub fn convert_depth_to_pressure<T>(
    pres: Vec<f32>,
    pres_qc: Vec<i8>,
    deph: Vec<f32>,
    deph_qc: Vec<i8>,
    fill_value: f32,
    latitude: Vec<T>,
) -> (Vec<f32>, Vec<i8>, Vec<i8>)
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
                    converted_pres_qc[i] = deph_qc[i];
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
    deph_qc: Vec<i8>,
    pres: Vec<f32>,
    pres_qc: Vec<i8>,
    fill_value: f32,
    latitude: Vec<T>,
) -> (Vec<f32>, Vec<i8>, Vec<i8>)
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
            converted_deph_qc[i] = pres_qc[i];
            deph_conv[i] = 1;
        }
    }

    (converted_deph, converted_deph_qc, deph_conv)
}
