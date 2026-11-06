use std::error::Error;
use std::path::Path;

use num_traits::{Float, FromPrimitive};
use chrono::{Duration, TimeZone, Utc};
use netcdf;
use netcdf::{NcTypeDescriptor, types::NcVariableType};
use gsw::conversions::{p_from_z, z_from_p};
use super::cora_config::QcType;
use super::source::{PlatformSource, SourceSpec};

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

/// Render a scalar attribute value as text: strings verbatim, numbers in their
/// shortest form. Arrays and empty strings yield `None`.
pub fn attribute_to_string(value: netcdf::AttributeValue) -> Option<String> {
    use netcdf::AttributeValue as A;
    let s = match value {
        A::Str(s) => s,
        A::Uchar(v) => v.to_string(),
        A::Schar(v) => v.to_string(),
        A::Ushort(v) => v.to_string(),
        A::Short(v) => v.to_string(),
        A::Uint(v) => v.to_string(),
        A::Int(v) => v.to_string(),
        A::Ulonglong(v) => v.to_string(),
        A::Longlong(v) => v.to_string(),
        A::Float(v) => v.to_string(),
        A::Double(v) => v.to_string(),
        _ => return None,
    };
    (!s.trim().is_empty()).then_some(s)
}

/// Platform codes for the profile slice `[prof_offset, prof_offset + prof_count)`,
/// each repeated `obs_len` times, read from where `spec.platform` says.
///
/// - A global attribute applies to every row. If it is missing or empty,
///   `defaults.platform_code` is used, else this is an error.
/// - A variable may be `char [profile, strlen]` (whitespace and NULs removed),
///   `string [profile]`, an integer `[profile]`, or a scalar of any of these
///   (one code for the whole file). Empty entries take `defaults.platform_code`
///   when set. A missing variable yields the default, or `" "` without one
///   (the historical CORA behaviour).
pub fn platform_codes_chunk(
    file: &netcdf::File,
    spec: &SourceSpec,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let n = prof_count * obs_len;
    let default = spec.defaults.platform_code.as_deref();

    let name = match &spec.platform {
        PlatformSource::Attribute(name) => {
            let value = file.attribute(name).and_then(|a| a.value().ok()).and_then(attribute_to_string);
            return match value.as_deref().or(default) {
                Some(v) => Ok(vec![v.to_string(); n]),
                None => Err(format!(
                    "global attribute \"{name}\" not found; set [defaults] platform_code in the mapping file"
                ).into()),
            };
        }
        PlatformSource::Variable(name) => name,
    };

    let var = match file.variable(name) {
        Some(v) => v,
        None => return Ok(vec![default.unwrap_or(" ").to_string(); n]),
    };
    let dims: Vec<String> = var.dimensions().iter().map(|d| d.name()).collect();
    let per_profile = dims.first().is_some_and(|d| *d == spec.profile_dim);

    // One string per profile in the slice (or a single file-wide one).
    let codes: Vec<String> = match var.vartype() {
        NcVariableType::Char if per_profile && dims.len() == 2 => {
            let strlen = var.dimensions()[1].len();
            get_char_value2_chunk(file, name, prof_offset, prof_count, 1, strlen)?
        }
        NcVariableType::Char if !per_profile && dims.len() <= 1 => {
            let chars: Vec<NcChar> = var.get_values::<NcChar, _>(..)?;
            let s: String = chars.iter().map(|&NcChar(c)| c as u8 as char).collect();
            vec![s.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string()]
        }
        NcVariableType::String if per_profile && dims.len() == 1 => (prof_offset..prof_offset + prof_count)
            .map(|i| var.get_string([i]).map(|s| s.trim().to_string()))
            .collect::<Result<_, _>>()?,
        NcVariableType::String if dims.is_empty() => vec![var.get_string(..)?.trim().to_string()],
        NcVariableType::Int(_) if per_profile && dims.len() == 1 => var
            .get_values::<i64, _>([prof_offset..prof_offset + prof_count])?
            .iter()
            .map(i64::to_string)
            .collect(),
        NcVariableType::Int(_) if dims.is_empty() => vec![var.get_values::<i64, _>(..)?[0].to_string()],
        _ => {
            return Err(format!(
                "platform variable \"{name}\" must be char [{p}, <strlen>], string [{p}] or integer [{p}] (or a scalar); found {:?} {dims:?}",
                var.vartype(),
                p = spec.profile_dim,
            ).into())
        }
    };

    let fill = |s: String| match default {
        Some(d) if s.is_empty() => d.to_string(),
        _ => s,
    };
    if !per_profile {
        return Ok(vec![fill(codes.into_iter().next().unwrap_or_default()); n]);
    }
    Ok(codes.into_iter().map(fill).flat_map(|s| std::iter::repeat_n(s, obs_len)).collect())
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

/// `ScanArgsParquet` with Polars' parallel Parquet reader disabled.
///
/// Polars 0.43's parallel (row-group) reader mishandles `slice` pushdown on a
/// **multi-row-group** Parquet file: every sliced window returns rows from the
/// first row group, silently corrupting the chunked streaming passes used by
/// `filter`/`dropqc`/`dropna`/`markdup`/`dedup` whenever their input has more
/// than one row group (e.g. any `concat`/`markdup` output over `chunk_rows()`
/// rows). `ParallelStrategy::None` slices correctly. Whole-file
/// group-by/`collect` (e.g. `report`, `concat`) is unaffected and keeps the
/// default parallel reader.
pub fn seq_scan_args() -> polars::prelude::ScanArgsParquet {
    polars::prelude::ScanArgsParquet {
        parallel: polars::prelude::ParallelStrategy::None,
        ..Default::default()
    }
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

/// Keep only the elements where `keep[i]` is `true`, consuming `v`. `v` and `keep`
/// must have the same length.
///
/// Converters drop all-NaN observation rows this way — on the raw column vectors,
/// *before* building the `DataFrame` — instead of Polars' `DataFrame::filter` /
/// `take`, which leak memory per call in the pinned Polars version (each gather of
/// the string columns retains memory that is never released). On a batch of many
/// files this leak accumulates without bound; filtering the vectors here avoids it
/// entirely and also keeps the dense all-NaN levels out of Polars.
pub fn retain_by_mask<T>(v: Vec<T>, keep: &[bool]) -> Vec<T> {
    debug_assert_eq!(v.len(), keep.len());
    v.into_iter()
        .zip(keep.iter())
        .filter_map(|(x, &k)| k.then_some(x))
        .collect()
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

/// A scalar numeric attribute as `f64`; strings, arrays and absent attributes
/// yield `None`.
fn numeric_attribute(var: &netcdf::Variable, name: &str) -> Option<f64> {
    use netcdf::AttributeValue as A;
    Some(match var.attribute_value(name)?.ok()? {
        A::Uchar(v) => v as f64,
        A::Schar(v) => v as f64,
        A::Ushort(v) => v as f64,
        A::Short(v) => v as f64,
        A::Uint(v) => v as f64,
        A::Int(v) => v as f64,
        A::Ulonglong(v) => v as f64,
        A::Longlong(v) => v as f64,
        A::Float(v) => v as f64,
        A::Double(v) => v,
        _ => return None,
    })
}

/// The NetCDF library's default fill value for a numeric variable type: what
/// unwritten cells read back as when no `_FillValue` attribute is set.
fn default_nc_fill(vartype: &NcVariableType) -> Option<f64> {
    use netcdf::types::{FloatType, IntType};
    // NC_FILL_* from netcdf.h.
    Some(match vartype {
        NcVariableType::Float(FloatType::F32) => 9.969_21e36_f32 as f64,
        NcVariableType::Float(FloatType::F64) => 9.969_209_968_386_869e36,
        NcVariableType::Int(IntType::I8) => -127.0,
        NcVariableType::Int(IntType::U8) => 255.0,
        NcVariableType::Int(IntType::I16) => -32_767.0,
        NcVariableType::Int(IntType::U16) => 65_535.0,
        NcVariableType::Int(IntType::I32) => -2_147_483_647.0,
        NcVariableType::Int(IntType::U32) => 4_294_967_295.0,
        _ => return None,
    })
}

/// The value that marks a missing cell of `var_name`, as `f64`.
///
/// Always honours `_FillValue` of any numeric type. With `cf_decode` it falls
/// back to `missing_value` and then to the NetCDF default fill for the type,
/// as CF readers do. `None` when the variable is absent or nothing applies.
pub fn fill_value_f64(file: &netcdf::File, var_name: &str, cf_decode: bool) -> Option<f64> {
    let var = file.variable(var_name)?;
    if let Some(v) = numeric_attribute(&var, "_FillValue") {
        return Some(v);
    }
    if !cf_decode {
        return None;
    }
    numeric_attribute(&var, "missing_value").or_else(|| default_nc_fill(&var.vartype()))
}

/// [`fill_value_f64`] as `f32`, with NaN standing for "no fill value".
pub fn get_float_fill_value(file: &netcdf::File, var_name: &str, cf_decode: bool) -> f32 {
    fill_value_f64(file, var_name, cf_decode).map_or(f32::NAN, |v| v as f32)
}

/// `(scale_factor, add_offset)` of a CF-packed variable, or `None` when it has
/// neither attribute.
fn packing(file: &netcdf::File, var_name: &str) -> Option<(f64, f64)> {
    let var = file.variable(var_name)?;
    let scale = numeric_attribute(&var, "scale_factor");
    let offset = numeric_attribute(&var, "add_offset");
    if scale.is_none() && offset.is_none() {
        return None;
    }
    Some((scale.unwrap_or(1.0), offset.unwrap_or(0.0)))
}

/// Read a 2-D `[profile, level]` measurement variable for the profile slice,
/// returning the values (fill cells as NaN) and the fill value the conversion
/// helpers compare against. With `spec.cf_decode`, packed values are unpacked.
pub fn get_param_chunk(
    file: &netcdf::File,
    var_name: &str,
    spec: &SourceSpec,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<(Vec<f32>, f32), Box<dyn Error>> {
    let fill = get_float_fill_value(file, var_name, spec.cf_decode);
    let mut values: Vec<f32> = get_var_float_value_chunk(file, var_name, fill, prof_offset, prof_count, obs_len)?;
    if spec.cf_decode {
        if let Some((scale, offset)) = packing(file, var_name) {
            for v in values.iter_mut() {
                *v = (*v as f64 * scale + offset) as f32;
            }
        }
    }
    Ok((values, fill))
}

/// Read a per-profile coordinate (time, longitude, latitude) for the profile
/// slice, tiled `obs_len` times. A missing variable yields `default` (NaN if
/// `None`). With `spec.cf_decode`, fill cells become NaN and packed values are
/// unpacked; without it the raw values are returned, as the Copernicus
/// converters always have.
pub fn get_coord_chunk<T>(
    file: &netcdf::File,
    var_name: &str,
    default: Option<f64>,
    spec: &SourceSpec,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<T>, Box<dyn Error>>
where
    T: Float + FromPrimitive + netcdf::NcTypeDescriptor,
{
    let missing = default.and_then(T::from_f64).unwrap_or_else(T::nan);
    let mut values: Vec<T> = get_coordinate_value_chunk(file, var_name, prof_offset, prof_count, obs_len, missing)?;
    if spec.cf_decode && file.variable(var_name).is_some() {
        let fill = fill_value_f64(file, var_name, true).and_then(T::from_f64);
        let packed = packing(file, var_name);
        for v in values.iter_mut() {
            if Some(*v) == fill {
                *v = T::nan();
            } else if let Some((scale, offset)) = packed {
                *v = T::from_f64(v.to_f64().unwrap_or(f64::NAN) * scale + offset).unwrap_or_else(T::nan);
            }
        }
    }
    Ok(values)
}

/// Profile time for the slice as days since 1950-01-01 (the `profile_time`
/// column), rescaled through `spec.time_scale`.
pub fn get_time_chunk(
    file: &netcdf::File,
    spec: &SourceSpec,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let mut time: Vec<f64> = get_coord_chunk(file, &spec.time, None, spec, prof_offset, prof_count, obs_len)?;
    if !spec.time_scale.is_identity() {
        for t in time.iter_mut() {
            *t = spec.time_scale.apply(*t);
        }
    }
    Ok(time)
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

/// Convert a raw integer QC flag (0–9) to its single-digit string.
/// Any value outside 0–9 (including the fill values) maps to `""`.
fn int_to_qc_string(v: i32) -> String {
    if (0..=9).contains(&v) {
        char::from(b'0' + v as u8).to_string()
    } else {
        String::new()
    }
}

/// The QC storage type of `var_name`: `qc_type` when given, else detected from
/// the variable (char variables are [`QcType::Char`], anything else
/// [`QcType::Int`]).
fn resolve_qc_type(file: &netcdf::File, var_name: &str, qc_type: Option<&QcType>) -> QcType {
    match qc_type {
        Some(t) => t.clone(),
        None => match file.variable(var_name).map(|v| v.vartype()) {
            Some(NcVariableType::Char) => QcType::Char,
            _ => QcType::Int,
        },
    }
}

/// Read a 2-D `[profile, level]` QC variable for the profile slice as one string
/// per flag, in the storage type `qc_type` (or the detected one when `None`).
pub fn read_qc_chunk(
    file: &netcdf::File,
    var_name: &str,
    qc_type: Option<&QcType>,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    match resolve_qc_type(file, var_name, qc_type) {
        QcType::Int => get_qc_value_chunk(file, var_name, prof_offset, prof_count, obs_len),
        QcType::Char => get_qc_value_from_char_chunk(file, var_name, prof_offset, prof_count, obs_len),
    }
}

/// Read a per-profile QC variable (`TIME_QC`, `POSITION_QC`) for the profile
/// slice, tiled `obs_len` times. `None` or a missing variable yields `""`.
pub fn read_qc_coord_chunk(
    file: &netcdf::File,
    var_name: Option<&str>,
    qc_type: Option<&QcType>,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let n = prof_count * obs_len;
    let var_name = match var_name {
        Some(v) if file.variable(v).is_some() => v,
        _ => return Ok(vec![String::new(); n]),
    };
    match resolve_qc_type(file, var_name, qc_type) {
        QcType::Int => get_qc_coordinate_value_chunk(file, var_name, prof_offset, prof_count, obs_len),
        QcType::Char => {
            let raw: Vec<NcChar> =
                get_coordinate_value_chunk(file, var_name, prof_offset, prof_count, obs_len, NcChar(b' ' as i8))?;
            Ok(raw
                .iter()
                .map(|&NcChar(c)| match c as u8 as char {
                    ' ' | '\0' => String::new(),
                    ch => ch.to_string(),
                })
                .collect())
        }
    }
}

/// Read the slice `[time_offset, time_offset + time_count)` of a 2-D `[TIME, DEPTH]`
/// integer QC variable and return each flag as a single-character string
/// ("0"–"9"). Read as `i32` so any integer storage type works (an `i8` read
/// of an `i16` variable would fail on its `-32767` fill). Missing variable or
/// out-of-range values → `""`.
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

    let raw: Vec<i32> = var_temp.get_values::<i32, _>([time_offset..time_offset + time_count, 0..obs_len])?;
    Ok(raw.iter().map(|&v| int_to_qc_string(v)).collect())
}

/// Read a coordinate-tiled integer QC variable (e.g., `TIME_QC`, `POSITION_QC`)
/// for the TIME slice `[time_offset, time_offset + time_count)` and return
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
    let raw: Vec<i32> = get_coordinate_value_chunk(file, var_name, time_offset, time_count, obs_len, i32::MIN)?;
    Ok(raw.iter().map(|&v| int_to_qc_string(v)).collect())
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

/// Data-mode value used when a file carries no data-mode information at all.
pub const DM_DEFAULT: &str = "R";

/// Map one data-mode character to its output string. The valid codes are
/// `R` (real-time), `A` (adjusted in real time) and `D` (delayed mode); anything
/// else (the `' '` fill value, `'\0'`, junk) is missing and maps to `""`.
pub fn dm_from_char(c: char) -> String {
    match c {
        'R' | 'A' | 'D' => c.to_string(),
        _ => String::new(),
    }
}

/// Resolve the single data-mode value that applies to a whole variable when its
/// per-observation `<PARAM>_DM` variable is absent: the variable's `data_mode`
/// attribute when it is one of `R`/`A`/`D`, else `fallback` (normally
/// [`DM_DEFAULT`]). A mixed (`"M"`) or missing attribute, or a missing
/// variable, also yields the fallback.
pub fn dm_from_attribute(attr: Option<&str>, fallback: &str) -> String {
    match attr.map(str::trim) {
        Some(s) if s.len() == 1 => {
            let dm = dm_from_char(s.chars().next().unwrap());
            if dm.is_empty() { fallback.to_string() } else { dm }
        }
        _ => fallback.to_string(),
    }
}

/// Read the `data_mode` string attribute of `var_name`, if both exist.
fn get_data_mode_attribute(file: &netcdf::File, var_name: &str) -> Option<String> {
    let var = file.variable(var_name)?;
    let value = var.attribute_value("data_mode")?.ok()?;
    get_string_value(value)
}

/// Read the per-observation data mode of `parent_var` for the TIME slice
/// `[time_offset, time_offset + time_count)`.
///
/// When the 2-D `[TIME, DEPTH]` char variable `dm_var` (e.g. `PRES_DM`) exists,
/// each cell is mapped through [`dm_from_char`] (fill cells become `""`). When it
/// is absent, every cell gets the value of `parent_var`'s `data_mode` attribute
/// (see [`dm_from_attribute`]), which defaults to `fallback`.
pub fn get_dm_value_chunk(
    file: &netcdf::File,
    dm_var: &str,
    parent_var: &str,
    fallback: &str,
    time_offset: usize,
    time_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let n = time_count * obs_len;
    if file.variable(dm_var).is_none() {
        let attr = get_data_mode_attribute(file, parent_var);
        return Ok(vec![dm_from_attribute(attr.as_deref(), fallback); n]);
    }
    let chars = get_char_value_chunk(file, dm_var, time_offset, time_count, obs_len)?;
    Ok(chars
        .iter()
        .map(|s| s.chars().next().map_or_else(String::new, dm_from_char))
        .collect())
}

/// Read a 1-D per-profile char data-mode variable (CORA's `DATA_MODE[N_PROF]`)
/// for the profile slice `[prof_offset, prof_offset + prof_count)` and tile each
/// value `obs_len` times. Fill cells become `""`; a missing variable yields
/// `fallback` (normally [`DM_DEFAULT`]) everywhere.
pub fn get_dm_coordinate_value_chunk(
    file: &netcdf::File,
    var_name: &str,
    fallback: &str,
    prof_offset: usize,
    prof_count: usize,
    obs_len: usize,
) -> Result<Vec<String>, Box<dyn Error>> {
    let n = prof_count * obs_len;
    if file.variable(var_name).is_none() {
        return Ok(vec![fallback.to_string(); n]);
    }
    let raw: Vec<NcChar> =
        get_coordinate_value_chunk(file, var_name, prof_offset, prof_count, obs_len, NcChar(b' ' as i8))?;
    Ok(raw.iter().map(|&NcChar(c)| dm_from_char(c as u8 as char)).collect())
}

/// Where `conv[i] == 1` (the pressure value was derived from depth), take the data
/// mode from `deph_dm`; elsewhere keep `pres_dm`. Mirrors how `pres_qc` inherits
/// `deph_qc` in [`convert_depth_to_pressure`].
pub fn merge_converted_dm(pres_dm: Vec<String>, deph_dm: &[String], conv: &[i8]) -> Vec<String> {
    pres_dm
        .into_iter()
        .zip(deph_dm.iter().zip(conv.iter()))
        .map(|(p, (d, &c))| if c == 1 { d.clone() } else { p })
        .collect()
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

    // ── data mode ────────────────────────────────────────────────────────────

    #[test]
    fn test_dm_from_char_valid_codes() {
        assert_eq!(dm_from_char('R'), "R");
        assert_eq!(dm_from_char('A'), "A");
        assert_eq!(dm_from_char('D'), "D");
    }

    #[test]
    fn test_dm_from_char_fill_and_junk_are_empty() {
        assert_eq!(dm_from_char(' '), "");
        assert_eq!(dm_from_char('\0'), "");
        assert_eq!(dm_from_char('M'), "");
        assert_eq!(dm_from_char('r'), "");
        assert_eq!(dm_from_char('1'), "");
    }

    #[test]
    fn test_dm_from_attribute_single_code_is_used() {
        assert_eq!(dm_from_attribute(Some("R"), DM_DEFAULT), "R");
        assert_eq!(dm_from_attribute(Some("A"), DM_DEFAULT), "A");
        assert_eq!(dm_from_attribute(Some("D"), DM_DEFAULT), "D");
        assert_eq!(dm_from_attribute(Some(" D "), "R"), "D");
    }

    #[test]
    fn test_dm_from_attribute_falls_back_to_default() {
        assert_eq!(dm_from_attribute(None, DM_DEFAULT), DM_DEFAULT);
        assert_eq!(dm_from_attribute(Some("M"), DM_DEFAULT), DM_DEFAULT);
        assert_eq!(dm_from_attribute(Some(""), DM_DEFAULT), DM_DEFAULT);
        assert_eq!(dm_from_attribute(Some(" "), DM_DEFAULT), DM_DEFAULT);
        assert_eq!(dm_from_attribute(Some("RD"), DM_DEFAULT), DM_DEFAULT);
    }

    #[test]
    fn test_dm_from_attribute_custom_fallback() {
        assert_eq!(dm_from_attribute(None, "D"), "D");
        assert_eq!(dm_from_attribute(Some("M"), "D"), "D");
        assert_eq!(dm_from_attribute(Some("A"), "D"), "A");
    }

    #[test]
    fn test_merge_converted_dm_takes_deph_dm_where_converted() {
        let pres_dm = vec!["R".to_string(), "R".to_string(), "".to_string()];
        let deph_dm = vec!["D".to_string(), "D".to_string(), "A".to_string()];
        let conv = vec![0_i8, 1, 1];
        assert_eq!(merge_converted_dm(pres_dm, &deph_dm, &conv), vec!["R", "D", "A"]);
    }

    // ── int_to_qc_string ─────────────────────────────────────────────────────

    #[test]
    fn test_int_to_qc_string_all_digits() {
        for d in 0i32..=9 {
            assert_eq!(int_to_qc_string(d), d.to_string(), "digit {d}");
        }
    }

    #[test]
    fn test_int_to_qc_string_fill_values_are_empty() {
        assert_eq!(int_to_qc_string(i8::MIN as i32), "");
        assert_eq!(int_to_qc_string(-127), "");
        assert_eq!(int_to_qc_string(-32_767), "");
        assert_eq!(int_to_qc_string(i32::MIN), "");
    }

    #[test]
    fn test_int_to_qc_string_negative_is_empty() {
        assert_eq!(int_to_qc_string(-1), "");
    }

    #[test]
    fn test_int_to_qc_string_above_9_is_empty() {
        assert_eq!(int_to_qc_string(10), "");
        assert_eq!(int_to_qc_string(100), "");
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
