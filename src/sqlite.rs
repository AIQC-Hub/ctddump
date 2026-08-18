//! Export a Parquet data file to a SQLite database.
//!
//! The flat observation-level Parquet is normalised into three tables:
//!
//! * `platform`    - one row per `platform_code` (an id and the code).
//! * `profile`     - one row per `(platform_code, profile_no)`, holding every
//!   column that is constant within a profile (time, position, QC, filename),
//!   with a foreign key to `platform`.
//! * `observation` - one row per observation (the measurements and their QC /
//!   conversion flags), with a foreign key to `profile`.
//!
//! `NaN` floats and empty QC strings are stored as SQL `NULL`. Only the columns
//! of the standard output schema are exported; anything else in the Parquet is
//! ignored unless requested with `--add-col`.
//!
//! Two streaming passes keep peak memory bounded like the other commands. Pass 1
//! groups by `(platform_code, profile_no)` to build the `platform` and `profile`
//! rows in memory (profile count, not file size, drives this, the same
//! assumption `markdup`/`dedup` rely on). Pass 2 re-streams the file in
//! `chunk_rows()` windows and inserts the observations, so the dense body never
//! materialises at once. Both scans use `common::seq_scan_args()`, required for
//! correct slicing of multi-row-group inputs (see the Polars notes in CLAUDE.md).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};

use polars::prelude::*;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

use crate::convert::common;

/// Options gathered from the CLI for a `sqlite` export.
pub struct Options {
    /// Overwrite the output file if it already exists.
    pub force: bool,
    /// Raw `TABLE.COL=VALUE` constant-column specs.
    pub add: Vec<String>,
    /// Raw `TABLE.COL` passthrough-column specs.
    pub add_col: Vec<String>,
}

/// Which table (and, equivalently, which grain) a column belongs to.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Table {
    Platform,
    Profile,
    Observation,
}

impl Table {
    fn name(self) -> &'static str {
        match self {
            Table::Platform => "platform",
            Table::Profile => "profile",
            Table::Observation => "observation",
        }
    }
}

/// The SQLite storage class chosen for a column.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum SqlType {
    Integer,
    Real,
    Text,
}

impl SqlType {
    fn as_str(self) -> &'static str {
        match self {
            SqlType::Integer => "INTEGER",
            SqlType::Real => "REAL",
            SqlType::Text => "TEXT",
        }
    }
}

/// Where an added column's values come from.
enum AddedSource {
    /// A constant value repeated on every row of the table.
    Literal(Value),
    /// Copied from an existing Parquet column of this name.
    Passthrough,
}

/// A user-requested extra column on one of the tables.
struct AddedColumn {
    name: String,
    sql_type: SqlType,
    source: AddedSource,
}

/// Standard profile-grain columns, in the order they appear in the table.
const PROFILE_STD: [&str; 9] = [
    "profile_time",
    "profile_timestamp",
    "longitude",
    "latitude",
    "profile_longitude",
    "profile_latitude",
    "time_qc",
    "position_qc",
    "filename",
];

/// Standard observation-grain columns, in the order they appear in the table.
const OBS_STD: [&str; 10] = [
    "temp", "temp_qc", "psal", "psal_qc", "pres", "pres_qc", "pres_conv", "deph", "deph_qc",
    "deph_conv",
];

/// SQLite storage class for a known standard column.
fn std_sql_type(name: &str) -> SqlType {
    match name {
        "profile_timestamp" | "pres_conv" | "deph_conv" => SqlType::Integer,
        "time_qc" | "position_qc" | "filename" => SqlType::Text,
        _ => SqlType::Real, // profile_time, positions, temp/psal/pres/deph
    }
}

/// Export `src` to a SQLite database at `dest` (or `src` with a `.sqlite`
/// extension when `dest` is `None`).
pub fn run(src: &Path, dest: Option<&Path>, opts: &Options) -> Result<(), Box<dyn Error>> {
    let dest_buf: PathBuf = match dest {
        Some(d) => d.to_path_buf(),
        None => src.with_extension("sqlite"),
    };

    if dest_buf.exists() {
        if opts.force {
            std::fs::remove_file(&dest_buf)
                .map_err(|e| format!("Cannot overwrite {}: {}", dest_buf.display(), e))?;
        } else {
            return Err(format!(
                "output {} already exists (use --force to overwrite)",
                dest_buf.display()
            )
            .into());
        }
    }

    let scan = || {
        LazyFrame::scan_parquet(src, common::seq_scan_args())
            .map_err(|e| format!("Cannot scan {}: {}", src.display(), e))
    };

    // Column names and dtypes of the input, from an empty (schema-only) slice.
    let empty = scan()?.slice(0, 0).collect()?;
    let input_schema = empty.schema();
    let present: BTreeSet<String> = empty
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    for key in ["platform_code", "profile_no", "observation_no"] {
        if !present.contains(key) {
            return Err(format!(
                "{} is missing the required column `{key}`; is it a ctddump data Parquet?",
                src.display()
            )
            .into());
        }
    }

    let profile_std: Vec<String> = PROFILE_STD
        .iter()
        .filter(|c| present.contains(**c))
        .map(|c| c.to_string())
        .collect();
    let obs_std: Vec<String> = OBS_STD
        .iter()
        .filter(|c| present.contains(**c))
        .map(|c| c.to_string())
        .collect();

    // Parse and validate the requested extra columns, one Vec per table.
    let added = parse_added(opts, &present, &input_schema, &profile_std, &obs_std)?;
    let platform_added = &added[&Table::Platform];
    let profile_added = &added[&Table::Profile];
    let obs_added = &added[&Table::Observation];

    // Passthrough column names by target grain (constancy is enforced for the
    // profile/platform ones during pass 1).
    let profile_pass: Vec<String> = passthrough_names(profile_added);
    let platform_pass: Vec<String> = passthrough_names(platform_added);
    let obs_pass: Vec<String> = passthrough_names(obs_added);

    let total = scan()?
        .select([len().alias("n")])
        .collect()?
        .column("n")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    // ── Pass 1: build platform and profile rows ─────────────────────────────
    let built = build_profiles(&scan, total, &profile_std, &profile_pass, &platform_pass)?;

    // Deterministic ids: platforms by sorted code, profiles by sorted
    // (code, profile_no) (the BTreeMap already iterates in that order).
    let mut platform_ids: BTreeMap<String, i64> = BTreeMap::new();
    for (pc, _pn) in built.profiles.keys() {
        platform_ids.entry(pc.clone()).or_insert(0);
    }
    for (i, id) in platform_ids.values_mut().enumerate() {
        *id = i as i64 + 1;
    }
    let mut profile_ids: HashMap<(String, i64), i64> = HashMap::new();
    for (i, key) in built.profiles.keys().enumerate() {
        profile_ids.insert(key.clone(), i as i64 + 1);
    }

    // ── Write the database ──────────────────────────────────────────────────
    let mut conn = Connection::open(&dest_buf)
        .map_err(|e| format!("Cannot create {}: {}", dest_buf.display(), e))?;
    // A freshly built export needs no crash safety; these make the load fast.
    conn.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;")?;
    conn.execute_batch(&create_sql(&profile_std, &obs_std, platform_added, profile_added, obs_added))?;

    write_platforms(&mut conn, &platform_ids, platform_added, &built.platform_pass)?;
    write_profiles(
        &mut conn,
        &built,
        &platform_ids,
        &profile_ids,
        &profile_std,
        &profile_pass,
        profile_added,
    )?;

    // ── Pass 2: stream and insert the observations ──────────────────────────
    write_observations(
        &mut conn, &scan, total, &profile_ids, &obs_std, &obs_pass, obs_added,
    )?;

    // Indexes last, so the bulk inserts above are not slowed by maintaining them.
    let mut idx = String::from(
        "CREATE INDEX idx_profile_platform ON profile(platform_id);\n\
         CREATE INDEX idx_observation_profile ON observation(profile_id);\n",
    );
    if profile_std.iter().any(|c| c == "profile_timestamp") {
        idx.push_str("CREATE INDEX idx_profile_timestamp ON profile(profile_timestamp);\n");
    }
    conn.execute_batch(&idx)?;

    Ok(())
}

/// Passthrough column names of an added-column list, in table order.
fn passthrough_names(added: &[AddedColumn]) -> Vec<String> {
    added
        .iter()
        .filter(|c| matches!(c.source, AddedSource::Passthrough))
        .map(|c| c.name.clone())
        .collect()
}

/// Parse every `--add` / `--add-col` spec into per-table [`AddedColumn`] lists,
/// validating table names, identifiers, source columns, and collisions. Within a
/// table, literal columns come first (in `--add` order) then passthrough columns
/// (in `--add-col` order), which is the order they are created and written in.
fn parse_added(
    opts: &Options,
    present: &BTreeSet<String>,
    input_schema: &Schema,
    profile_std: &[String],
    obs_std: &[String],
) -> Result<BTreeMap<Table, Vec<AddedColumn>>, Box<dyn Error>> {
    let mut out: BTreeMap<Table, Vec<AddedColumn>> = BTreeMap::new();
    out.insert(Table::Platform, Vec::new());
    out.insert(Table::Profile, Vec::new());
    out.insert(Table::Observation, Vec::new());

    // Reserved (base + standard) column names per table that an added column may
    // not shadow.
    let reserved = |table: Table| -> BTreeSet<String> {
        let mut s: BTreeSet<String> = BTreeSet::new();
        match table {
            Table::Platform => {
                s.insert("platform_id".into());
                s.insert("platform_code".into());
            }
            Table::Profile => {
                s.extend(["profile_id", "platform_id", "profile_no"].map(String::from));
                s.extend(profile_std.iter().cloned());
            }
            Table::Observation => {
                s.extend(["observation_id", "profile_id", "observation_no"].map(String::from));
                s.extend(obs_std.iter().cloned());
            }
        }
        s
    };

    let check_new = |table: Table, name: &str, out: &BTreeMap<Table, Vec<AddedColumn>>| -> Result<(), Box<dyn Error>> {
        if !is_ident(name) {
            return Err(format!("invalid column name `{name}` (letters, digits, and underscore only, not starting with a digit)").into());
        }
        if reserved(table).contains(name) {
            return Err(format!("cannot add column `{name}` to `{}`: it is already a standard column", table.name()).into());
        }
        if out[&table].iter().any(|c| c.name == name) {
            return Err(format!("column `{name}` added to `{}` more than once", table.name()).into());
        }
        Ok(())
    };

    for spec in &opts.add {
        let (key, value) = spec
            .split_once('=')
            .ok_or_else(|| format!("--add expects TABLE.COL=VALUE, got `{spec}`"))?;
        let (table, name) = parse_target(key)?;
        check_new(table, name, &out)?;
        let (sql_type, val) = infer_literal(value);
        out.get_mut(&table).unwrap().push(AddedColumn {
            name: name.to_string(),
            sql_type,
            source: AddedSource::Literal(val),
        });
    }

    for spec in &opts.add_col {
        let (table, name) = parse_target(spec)?;
        check_new(table, name, &out)?;
        if !present.contains(name) {
            return Err(format!("--add-col: column `{name}` is not in the input Parquet").into());
        }
        let dtype = input_schema
            .get(name)
            .ok_or_else(|| format!("--add-col: column `{name}` is not in the input Parquet"))?;
        out.get_mut(&table).unwrap().push(AddedColumn {
            name: name.to_string(),
            sql_type: sql_type_from_dtype(dtype),
            source: AddedSource::Passthrough,
        });
    }

    Ok(out)
}

/// Split a `TABLE.COL` target into its table and column name.
fn parse_target(key: &str) -> Result<(Table, &str), Box<dyn Error>> {
    let (table_s, col) = key
        .split_once('.')
        .ok_or_else(|| format!("expected TABLE.COL, got `{key}`"))?;
    let table = match table_s {
        "platform" => Table::Platform,
        "profile" => Table::Profile,
        "observation" => Table::Observation,
        other => {
            return Err(format!(
                "unknown table `{other}` (expected platform, profile, or observation)"
            )
            .into())
        }
    };
    if col.is_empty() {
        return Err(format!("empty column name in `{key}`").into());
    }
    Ok((table, col))
}

/// Whether `s` is a safe SQL identifier (also guards the quoted-identifier SQL).
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Infer a literal value's SQLite storage class: integer, then real, else text.
fn infer_literal(v: &str) -> (SqlType, Value) {
    if let Ok(i) = v.parse::<i64>() {
        return (SqlType::Integer, Value::Integer(i));
    }
    if let Ok(f) = v.parse::<f64>() {
        if f.is_finite() {
            return (SqlType::Real, Value::Real(f));
        }
    }
    (SqlType::Text, Value::Text(v.to_string()))
}

/// Map a Polars dtype to the SQLite storage class used for a passthrough column.
fn sql_type_from_dtype(dt: &DataType) -> SqlType {
    match dt {
        DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Datetime(_, _) => SqlType::Integer,
        DataType::Float32 | DataType::Float64 => SqlType::Real,
        _ => SqlType::Text,
    }
}

/// Convert one Polars cell to a SQLite value. `NaN` floats and empty strings
/// become `NULL`; datetimes become their integer (millisecond) value.
fn any_to_sql(av: AnyValue) -> Value {
    match av {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(b) => Value::Integer(b as i64),
        AnyValue::Int8(v) => Value::Integer(v as i64),
        AnyValue::Int16(v) => Value::Integer(v as i64),
        AnyValue::Int32(v) => Value::Integer(v as i64),
        AnyValue::Int64(v) => Value::Integer(v),
        AnyValue::UInt8(v) => Value::Integer(v as i64),
        AnyValue::UInt16(v) => Value::Integer(v as i64),
        AnyValue::UInt32(v) => Value::Integer(v as i64),
        AnyValue::UInt64(v) => Value::Integer(v as i64),
        AnyValue::Float32(v) => {
            if v.is_nan() {
                Value::Null
            } else {
                Value::Real(v as f64)
            }
        }
        AnyValue::Float64(v) => {
            if v.is_nan() {
                Value::Null
            } else {
                Value::Real(v)
            }
        }
        AnyValue::String(s) => {
            if s.is_empty() {
                Value::Null
            } else {
                Value::Text(s.to_string())
            }
        }
        AnyValue::StringOwned(s) => {
            if s.is_empty() {
                Value::Null
            } else {
                Value::Text(s.to_string())
            }
        }
        AnyValue::Datetime(v, _, _) => Value::Integer(v),
        other => Value::Text(other.to_string()),
    }
}

/// The in-memory result of pass 1.
struct Built {
    /// Per profile, keyed by `(platform_code, profile_no)` in sorted order.
    profiles: BTreeMap<(String, i64), ProfileData>,
    /// Per platform passthrough values, keyed by `platform_code`, aligned to the
    /// platform table's passthrough columns.
    platform_pass: BTreeMap<String, Vec<Value>>,
}

/// Stored values for one profile row.
struct ProfileData {
    /// Standard profile columns, aligned to the present `PROFILE_STD` order.
    std: Vec<Value>,
    /// Profile-table passthrough values, aligned to the profile passthrough list.
    pass: Vec<Value>,
}

/// Pass 1: group by `(platform_code, profile_no)` and assemble the profile and
/// platform rows, enforcing that every profile/platform passthrough column is
/// constant within its group (nulls are ignored). Errors name the offending
/// column and group.
fn build_profiles<F>(
    scan: &F,
    total: usize,
    profile_std: &[String],
    profile_pass: &[String],
    platform_pass: &[String],
) -> Result<Built, Box<dyn Error>>
where
    F: Fn() -> Result<LazyFrame, String>,
{
    // min == max over a group means constant (aggregations skip nulls), so one
    // min/max pair per distinct passthrough column detects intra-group variation.
    let pass_cols: BTreeSet<String> = profile_pass
        .iter()
        .chain(platform_pass.iter())
        .cloned()
        .collect();

    let mut profiles: BTreeMap<(String, i64), ProfileData> = BTreeMap::new();
    let mut platform_pass_vals: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);

        let mut aggs: Vec<Expr> = Vec::new();
        for c in profile_std {
            aggs.push(col(c).first().alias(c));
        }
        for c in &pass_cols {
            aggs.push(col(c).min().alias(format!("__min__{c}")));
            aggs.push(col(c).max().alias(format!("__max__{c}")));
        }

        let g = scan()?
            .slice(offset as i64, count as IdxSize)
            .group_by([col("platform_code"), col("profile_no").cast(DataType::Int64).alias("profile_no")])
            .agg(aggs)
            .collect()?;

        let pc = g.column("platform_code")?.str()?;
        let pn = g.column("profile_no")?.i64()?;

        for i in 0..g.height() {
            let code = pc.get(i).unwrap_or("").to_string();
            let no = pn.get(i).unwrap_or(0);

            // Per-passthrough-column constant value for this group (min == max).
            let group_val = |c: &str| -> Result<Value, Box<dyn Error>> {
                let mn = sql_cell(&g, &format!("__min__{c}"), i)?;
                let mx = sql_cell(&g, &format!("__max__{c}"), i)?;
                if mn != mx {
                    return Err(format!(
                        "column `{c}` is not constant within profile ({code}, {no}); it cannot be added to a profile/platform table"
                    )
                    .into());
                }
                Ok(mn)
            };

            let key = (code.clone(), no);
            if let Some(existing) = profiles.get_mut(&key) {
                // Cross-chunk revisit: standard columns are constant so keep the
                // stored ones; merge passthrough values (null means "unknown").
                for (slot, c) in existing.pass.iter_mut().zip(profile_pass) {
                    merge_const(slot, group_val(c)?, c, &code, no)?;
                }
            } else {
                let std: Vec<Value> = profile_std
                    .iter()
                    .map(|c| Ok(sql_cell(&g, c, i)?))
                    .collect::<Result<_, Box<dyn Error>>>()?;
                let pass: Vec<Value> = profile_pass
                    .iter()
                    .map(|c| group_val(c))
                    .collect::<Result<_, Box<dyn Error>>>()?;
                profiles.insert(key, ProfileData { std, pass });
            }

            // Platform passthrough columns must be constant across the whole
            // platform, so merge each group's value into the platform slot.
            if !platform_pass.is_empty() {
                let slot = platform_pass_vals
                    .entry(code.clone())
                    .or_insert_with(|| vec![Value::Null; platform_pass.len()]);
                for (j, c) in platform_pass.iter().enumerate() {
                    let v = group_val(c)?;
                    merge_const(&mut slot[j], v, c, &code, no)?;
                }
            }
        }
        offset += count;
    }

    Ok(Built {
        profiles,
        platform_pass: platform_pass_vals,
    })
}

/// Read cell `i` of column `name` from `df` and convert it to a SQLite value.
fn sql_cell(df: &DataFrame, name: &str, i: usize) -> PolarsResult<Value> {
    Ok(any_to_sql(df.column(name)?.get(i)?))
}

/// Merge a newly seen constant value into a stored slot. A stored/new `NULL` is
/// "unknown" and yields to the other; two differing non-null values are an error.
fn merge_const(
    slot: &mut Value,
    new: Value,
    col: &str,
    code: &str,
    no: i64,
) -> Result<(), Box<dyn Error>> {
    match (&*slot, &new) {
        (Value::Null, _) => *slot = new,
        (_, Value::Null) => {}
        (a, b) if a == b => {}
        _ => {
            return Err(format!(
                "column `{col}` is not constant across platform `{code}` (differs by profile {no})"
            )
            .into())
        }
    }
    Ok(())
}

/// Build the `CREATE TABLE` statements for all three tables.
fn create_sql(
    profile_std: &[String],
    obs_std: &[String],
    platform_added: &[AddedColumn],
    profile_added: &[AddedColumn],
    obs_added: &[AddedColumn],
) -> String {
    let added_cols = |added: &[AddedColumn]| -> String {
        added
            .iter()
            .map(|c| format!(",\n  \"{}\" {}", c.name, c.sql_type.as_str()))
            .collect::<String>()
    };
    let std_cols = |cols: &[String]| -> String {
        cols.iter()
            .map(|c| format!(",\n  \"{}\" {}", c, std_sql_type(c).as_str()))
            .collect::<String>()
    };

    format!(
        "CREATE TABLE platform (\n  \
           platform_id INTEGER PRIMARY KEY,\n  \
           platform_code TEXT UNIQUE NOT NULL{platform_added}\n);\n\
         CREATE TABLE profile (\n  \
           profile_id INTEGER PRIMARY KEY,\n  \
           platform_id INTEGER NOT NULL REFERENCES platform(platform_id),\n  \
           profile_no INTEGER NOT NULL{profile_std}{profile_added},\n  \
           UNIQUE(platform_id, profile_no)\n);\n\
         CREATE TABLE observation (\n  \
           observation_id INTEGER PRIMARY KEY,\n  \
           profile_id INTEGER NOT NULL REFERENCES profile(profile_id),\n  \
           observation_no INTEGER NOT NULL{obs_std}{obs_added}\n);\n",
        platform_added = added_cols(platform_added),
        profile_std = std_cols(profile_std),
        profile_added = added_cols(profile_added),
        obs_std = std_cols(obs_std),
        obs_added = added_cols(obs_added),
    )
}

/// Comma-separated quoted column list for an `INSERT`.
fn quoted(cols: &[String]) -> String {
    cols.iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `?, ?, ...` placeholders for `n` values.
fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// Insert the `platform` rows.
fn write_platforms(
    conn: &mut Connection,
    platform_ids: &BTreeMap<String, i64>,
    platform_added: &[AddedColumn],
    platform_pass: &BTreeMap<String, Vec<Value>>,
) -> Result<(), Box<dyn Error>> {
    let mut cols = vec!["platform_id".to_string(), "platform_code".to_string()];
    cols.extend(platform_added.iter().map(|c| c.name.clone()));
    let sql = format!(
        "INSERT INTO platform ({}) VALUES ({})",
        quoted(&cols),
        placeholders(cols.len())
    );

    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(&sql)?;
        for (code, id) in platform_ids {
            let mut vals: Vec<Value> = vec![Value::Integer(*id), Value::Text(code.clone())];
            let mut pass_i = 0;
            for c in platform_added {
                match &c.source {
                    AddedSource::Literal(v) => vals.push(v.clone()),
                    AddedSource::Passthrough => {
                        let v = platform_pass
                            .get(code)
                            .and_then(|vs| vs.get(pass_i))
                            .cloned()
                            .unwrap_or(Value::Null);
                        vals.push(v);
                        pass_i += 1;
                    }
                }
            }
            stmt.execute(params_from_iter(vals.iter()))?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Insert the `profile` rows.
fn write_profiles(
    conn: &mut Connection,
    built: &Built,
    platform_ids: &BTreeMap<String, i64>,
    profile_ids: &HashMap<(String, i64), i64>,
    profile_std: &[String],
    profile_pass: &[String],
    profile_added: &[AddedColumn],
) -> Result<(), Box<dyn Error>> {
    let mut cols = vec![
        "profile_id".to_string(),
        "platform_id".to_string(),
        "profile_no".to_string(),
    ];
    cols.extend(profile_std.iter().cloned());
    cols.extend(profile_added.iter().map(|c| c.name.clone()));
    let sql = format!(
        "INSERT INTO profile ({}) VALUES ({})",
        quoted(&cols),
        placeholders(cols.len())
    );

    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(&sql)?;
        for (key, data) in &built.profiles {
            let (code, no) = key;
            let mut vals: Vec<Value> = vec![
                Value::Integer(profile_ids[key]),
                Value::Integer(platform_ids[code]),
                Value::Integer(*no),
            ];
            vals.extend(data.std.iter().cloned());
            let mut pass_i = 0;
            for c in profile_added {
                match &c.source {
                    AddedSource::Literal(v) => vals.push(v.clone()),
                    AddedSource::Passthrough => {
                        vals.push(data.pass.get(pass_i).cloned().unwrap_or(Value::Null));
                        pass_i += 1;
                    }
                }
            }
            debug_assert_eq!(pass_i, profile_pass.len());
            stmt.execute(params_from_iter(vals.iter()))?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Pass 2: stream the observations in `chunk_rows()` windows and insert them,
/// resolving each row's `profile_id` from the pass-1 map.
fn write_observations<F>(
    conn: &mut Connection,
    scan: &F,
    total: usize,
    profile_ids: &HashMap<(String, i64), i64>,
    obs_std: &[String],
    obs_pass: &[String],
    obs_added: &[AddedColumn],
) -> Result<(), Box<dyn Error>>
where
    F: Fn() -> Result<LazyFrame, String>,
{
    let mut cols = vec!["profile_id".to_string(), "observation_no".to_string()];
    cols.extend(obs_std.iter().cloned());
    cols.extend(obs_added.iter().map(|c| c.name.clone()));
    let sql = format!(
        "INSERT INTO observation ({}) VALUES ({})",
        quoted(&cols),
        placeholders(cols.len())
    );

    // Selected columns: keys (cast to i64), then the standard and passthrough
    // observation columns as-is.
    let mut select: Vec<Expr> = vec![
        col("platform_code"),
        col("profile_no").cast(DataType::Int64).alias("profile_no"),
        col("observation_no").cast(DataType::Int64).alias("observation_no"),
    ];
    for c in obs_std {
        select.push(col(c));
    }
    for c in obs_pass {
        select.push(col(c));
    }

    let step = common::chunk_rows();
    let mut offset = 0usize;
    while offset < total {
        let count = step.min(total - offset);
        let df = scan()?
            .slice(offset as i64, count as IdxSize)
            .select(select.clone())
            .collect()?;

        let pc = df.column("platform_code")?.str()?;
        let pn = df.column("profile_no")?.i64()?;
        let on = df.column("observation_no")?.i64()?;

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(&sql)?;
            for i in 0..df.height() {
                let key = (pc.get(i).unwrap_or("").to_string(), pn.get(i).unwrap_or(0));
                let profile_id = *profile_ids.get(&key).ok_or_else(|| {
                    format!("observation references unknown profile ({}, {})", key.0, key.1)
                })?;

                let mut vals: Vec<Value> = vec![
                    Value::Integer(profile_id),
                    Value::Integer(on.get(i).unwrap_or(0)),
                ];
                for c in obs_std {
                    vals.push(sql_cell(&df, c, i)?);
                }
                let mut pass_i = 0;
                for c in obs_added {
                    match &c.source {
                        AddedSource::Literal(v) => vals.push(v.clone()),
                        AddedSource::Passthrough => {
                            vals.push(sql_cell(&df, &obs_pass[pass_i], i)?);
                            pass_i += 1;
                        }
                    }
                }
                stmt.execute(params_from_iter(vals.iter()))?;
            }
        }
        tx.commit()?;
        offset += count;
    }
    Ok(())
}
