//! Render a report `DataFrame` as TSV, aligned plain text, or JSON, to a file or
//! stdout. Kept self-contained (no extra Polars IO features) so cell formatting —
//! float precision, datetimes, NaN/null → empty — is fully under our control.

use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use chrono::{DateTime, Utc};
use polars::prelude::*;

use crate::cli::ReportFormat;

/// Write `df` to `dest` (or stdout when `None`) in the requested format.
pub fn write_report(
    df: &DataFrame,
    format: ReportFormat,
    dest: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let mut w: Box<dyn Write> = match dest {
        Some(p) => Box::new(
            File::create(p).map_err(|e| format!("Cannot create {}: {}", p.display(), e))?,
        ),
        None => Box::new(io::stdout().lock()),
    };
    match format {
        ReportFormat::Tsv => write_tsv(df, &mut *w)?,
        ReportFormat::Text => write_text(df, &mut *w)?,
        ReportFormat::Json => write_json(df, &mut *w)?,
    }
    w.flush()?;
    Ok(())
}

fn write_tsv(df: &DataFrame, w: &mut dyn Write) -> Result<(), Box<dyn Error>> {
    let cols = df.get_columns();
    let header: Vec<String> = cols.iter().map(|s| s.name().to_string()).collect();
    writeln!(w, "{}", header.join("\t"))?;
    for r in 0..df.height() {
        let mut cells = Vec::with_capacity(cols.len());
        for s in cols {
            cells.push(av_to_string(&s.get(r)?));
        }
        writeln!(w, "{}", cells.join("\t"))?;
    }
    Ok(())
}

fn write_text(df: &DataFrame, w: &mut dyn Write) -> Result<(), Box<dyn Error>> {
    let cols = df.get_columns();

    // A single-row report (e.g. `--level global`) reads best as a label/value block.
    if df.height() == 1 {
        let label_w = cols.iter().map(|s| s.name().len()).max().unwrap_or(0);
        for s in cols {
            writeln!(w, "{:<label_w$}  {}", s.name().to_string(), av_to_string(&s.get(0)?))?;
        }
        return Ok(());
    }

    // Otherwise, an aligned table.
    let names: Vec<String> = cols.iter().map(|s| s.name().to_string()).collect();
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(df.height());
    for r in 0..df.height() {
        let mut row = Vec::with_capacity(cols.len());
        for s in cols {
            row.push(av_to_string(&s.get(r)?));
        }
        rows.push(row);
    }
    let widths: Vec<usize> = (0..cols.len())
        .map(|i| rows.iter().map(|row| row[i].len()).chain([names[i].len()]).max().unwrap_or(0))
        .collect();

    let fmt_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<w$}", c, w = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    writeln!(w, "{}", fmt_row(&names))?;
    let sep: Vec<String> = widths.iter().map(|wd| "-".repeat(*wd)).collect();
    writeln!(w, "{}", sep.join("  "))?;
    for row in &rows {
        writeln!(w, "{}", fmt_row(row))?;
    }
    Ok(())
}

fn write_json(df: &DataFrame, w: &mut dyn Write) -> Result<(), Box<dyn Error>> {
    let cols = df.get_columns();
    let names: Vec<String> = cols.iter().map(|s| s.name().to_string()).collect();
    write!(w, "[")?;
    for r in 0..df.height() {
        write!(w, "{}{{", if r == 0 { "" } else { "," })?;
        for (i, s) in cols.iter().enumerate() {
            write!(
                w,
                "{}{}:{}",
                if i == 0 { "" } else { "," },
                json_quote(&names[i]),
                av_to_json(&s.get(r)?)
            )?;
        }
        write!(w, "}}")?;
    }
    writeln!(w, "]")?;
    Ok(())
}

/// Format a floating-point value for display: NaN → empty, otherwise up to 4
/// decimals with trailing zeros trimmed.
fn fmt_float(v: f64) -> String {
    if v.is_nan() {
        return String::new();
    }
    let s = format!("{:.4}", v);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn fmt_datetime_ms(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

/// Cell → plain string (for TSV / text). Null and NaN render as empty.
fn av_to_string(av: &AnyValue) -> String {
    match av {
        AnyValue::Null => String::new(),
        AnyValue::String(s) => s.to_string(),
        AnyValue::StringOwned(s) => s.to_string(),
        AnyValue::Boolean(b) => b.to_string(),
        AnyValue::Float32(v) => fmt_float(*v as f64),
        AnyValue::Float64(v) => fmt_float(*v),
        AnyValue::Datetime(v, TimeUnit::Milliseconds, _) => fmt_datetime_ms(*v),
        other => other.to_string(),
    }
}

/// Cell → JSON value token (numbers unquoted, strings quoted/escaped, NaN/null → null).
fn av_to_json(av: &AnyValue) -> String {
    match av {
        AnyValue::Null => "null".to_string(),
        AnyValue::Boolean(b) => b.to_string(),
        AnyValue::String(s) => json_quote(s),
        AnyValue::StringOwned(s) => json_quote(s.as_str()),
        AnyValue::Float32(v) => json_num(*v as f64),
        AnyValue::Float64(v) => json_num(*v),
        AnyValue::Datetime(v, TimeUnit::Milliseconds, _) => json_quote(&fmt_datetime_ms(*v)),
        // Integer / other numeric AnyValues Display as a bare number.
        other => other.to_string(),
    }
}

fn json_num(v: f64) -> String {
    if v.is_nan() {
        "null".to_string()
    } else {
        fmt_float(v)
    }
}

fn json_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
