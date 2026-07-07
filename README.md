# ctddump

A Rust CLI tool for converting oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF to Parquet (data) or YAML (metadata).

Two data sources are supported:

| Source | Description |
|--------|-------------|
| **NRT** | Near Real Time — Arctic Sea (`nrt_ar`), Baltic Sea (`nrt_bo`), Mediterranean Sea (`nrt_mo`), Global (`nrt_gl`) |
| **CORA** | Copernicus Ocean Reanalysis — current format (`cora`), legacy format (`cora_legacy`) |

## Installation

### System dependencies

The `netcdf` crate links against the HDF5 C library:

```bash
# Ubuntu / Debian
sudo apt-get install libhdf5-dev libnetcdf-dev

# macOS (Homebrew)
brew install hdf5
```

### Build

```bash
git clone https://github.com/AIQC-Hub/ctddump.git
cd ctddump
cargo build --release
```

The binary is placed at `target/release/ctddump`.

## Commands

Every command and subcommand supports `-h` / `--help`.

### `convert` — single NetCDF → Parquet

```
ctddump convert <subcommand> [OPTIONS] <src_file> <target_file>
```

```bash
ctddump convert nrt_ar   input.nc output.parquet
ctddump convert nrt_bo   input.nc output.parquet
ctddump convert nrt_mo   input.nc output.parquet
ctddump convert nrt_gl   input.nc output.parquet
ctddump convert cora     input.nc output.parquet
ctddump convert cora_legacy input.nc output.parquet

# Use a saved config preset
ctddump convert nrt_ar --config my_preset.toml input.nc output.parquet

# Override individual fields
ctddump convert nrt_bo --no-deph-source input.nc output.parquet
```

### `batch` — directory tree NetCDF → Parquet or YAML (multi-threaded)

```
ctddump batch convert <subcommand> [OPTIONS] <src_dir>
ctddump batch header  <subcommand> [OPTIONS] <src_dir>
```

Recursively finds all matching `.nc` files and processes them in parallel. Output files keep the source stem with a new extension. If `--output` is omitted, each output is written alongside its source.

```bash
# Convert all NRT Arctic files to Parquet, flat into /output
ctddump batch convert nrt_ar --output /output /data/arctic

# Limit to 4 threads
ctddump batch convert nrt_ar --threads 4 --output /output /data/arctic

# Override the filename pattern
ctddump batch convert nrt_ar --pattern "AR_PR_CT_ITP-*.nc" --output /output /data

# Extract YAML metadata for all NRT files
ctddump batch header nrt --output /output /data/arctic
```

Default filename patterns:

| Subcommand | Pattern |
|------------|---------|
| `nrt_ar` | `AR_PR_CT_*.nc` |
| `nrt_bo` | `BO_PR_CT_*.nc` |
| `nrt_mo` | `MO_PR_CT_*.nc` |
| `nrt_gl` | `GL_PR_CT_*.nc` |
| `cora`, `cora_legacy` | `*.nc` |
| `batch header nrt`, `batch header cora` | `*.nc` |

### `header` — single NetCDF → YAML metadata

```
ctddump header <subcommand> <src_file> <target_file>
```

```bash
ctddump header nrt  input.nc output.yaml
ctddump header cora input.nc output.yaml
```

### `concat` — merge files from a directory tree into a single file

```
ctddump concat convert [OPTIONS] <src_dir> <output_file>
ctddump concat header  [OPTIONS] <src_dir> <output_file>
```

`concat convert` merges Parquet files and re-assigns `profile_no` and `observation_no` by default (pass `--no-renumber` to skip). Renumbering sorts rows by `platform_code, profile_timestamp, longitude, latitude, pres`; pass `--no-pres-sort` to sort without `pres`, keeping each profile's observations in their original source order instead of reordering them by pressure.

`concat header` merges YAML header files — each file contributes its top-level keys to the combined output. An error is raised if any two files share the same key.

```bash
# Merge all Parquet files with profile renumbering
ctddump concat convert /data/parquet merged.parquet

# Merge without renumbering
ctddump concat convert --no-renumber /data/parquet merged.parquet

# Merge, but keep each profile's observations in their original order (don't sort by pres)
ctddump concat convert --no-pres-sort /data/parquet merged.parquet

# Merge only a subset
ctddump concat convert --pattern "AR_PR_CT_*.parquet" /data/parquet merged.parquet

# Merge YAML headers
ctddump concat header /data/yaml merged.yaml
```

## Configuration

All `convert` and `batch convert` subcommands support a `--config` TOML file plus individual flag overrides. Priority order:

```
built-in default  <  --config file  <  individual CLI flags
```

### NRT flags

| Flag | Field | Default |
|------|-------|---------|
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` for BO/GL, `false` for AR/MO |
| `--profile-coords` / `--no-profile-coords` | `has_profile_coords` | `true` for BO, `false` otherwise |
| `--pattern <GLOB>` | `pattern` | see table above |

### CORA flags

| Flag | Field | `cora` default | `cora_legacy` default |
|------|-------|---------------|----------------------|
| `--time-var <VAR>` | `time_var` | `TIME` | `JULD` |
| `--qc-type <int\|char>` | `qc_type` | `int` | `char` |
| `--time-qc` / `--no-time-qc` | `has_time_qc` | `true` | `false` |
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` | `false` |
| `--pattern <GLOB>` | `pattern` | `*.nc` | `*.nc` |

### TOML config file format

```toml
# NRT
has_deph_source    = true
has_profile_coords = false
pattern            = "AR_PR_CT_*.nc"  # optional

# CORA
time_var      = "TIME"
qc_type       = "int"    # "int" or "char"
has_time_qc   = true
has_deph_source = true
pattern       = "*.nc"   # optional
```

## Output schema

All converters produce a uniform, observation-level flat table:

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
| `longitude` / `latitude` | `f32` (NRT) / `f64` (CORA) | |
| `profile_longitude` / `profile_latitude` | `f32` (NRT) / `f64` (CORA) | from `PRECISE_*` or expanded `DEPLOY_*`; NaN when unavailable |
| `time_qc` / `position_qc` | `String` | `""` if absent |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `String` | single-char flag; `""` if missing |
| `pres_conv`, `deph_conv` | `i8` | `1` = derived by conversion |

## Development

```bash
# Run all tests
cargo test

# Run a single test
cargo test test_convert_nrt_ar_1

# Download test fixture files (requires gh CLI, authenticated)
scripts/fetch_test_data.sh
```

> **Note:** HDF5-DIAG messages may appear in test output on systems with HDF5 ≤ 1.10. They are harmless — the data is read correctly and all tests pass.
