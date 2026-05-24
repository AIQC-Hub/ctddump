# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`ctddump` is a Rust CLI tool for converting oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF format to Parquet (data) or YAML (metadata). It targets data from two oceanographic repositories:
- **NRT** (Near Real Time): data from various regions — Arctic Sea (AR), Baltic Sea (BO), Mediterranean Sea (MO), and Global (GL)
- **CORA** (Copernicus Ocean Reanalysis): historical re-processed CTD profiles (`cora` and `cora2` for older files)

## Commands

```bash
# Build
cargo build

# Run all tests
cargo test

# Run a single test by name
cargo test test_netcdf_nrt_ar_1

# Build with trace feature enabled
cargo build --features trace

# Run the binary
cargo run -- <module> [args...]
# e.g.:
cargo run -- netcdf nrt_ar input.nc output.parquet
cargo run -- netcdf nrt_head input.nc output.yaml
```

### Test Data

Tests require fixture files in `tests/`. These are not committed to the repo — download them via:

```bash
scripts/fetch_test_data.sh
# Requires: gh CLI (authenticated) and unzip
# Optionally override: TEST_DATA_VERSION=v0.1.0 TEST_DATA_REPO=AIQC-Hub/ctddump
```

## CLI Interface

```
ctddump <module> [subcommand] <src_file> <target_file>
```

| Module | Subcommand | Input | Output |
|--------|------------|-------|--------|
| `netcdf` | `nrt_ar` | NRT Arctic Sea `.nc` | `.parquet` |
| `netcdf` | `nrt_bo` | NRT Baltic Sea `.nc` | `.parquet` |
| `netcdf` | `nrt_mo` | NRT Mediterranean Sea `.nc` | `.parquet` |
| `netcdf` | `nrt_gl` | NRT Global `.nc` | `.parquet` |
| `netcdf` | `nrt_head` | Any NRT `.nc` | `.yaml` metadata |
| `netcdf` | `cora` | CORA `.nc` | `.parquet` |
| `netcdf` | `cora2` | Older CORA `.nc` | `.parquet` |
| `netcdf` | `cora_head` | CORA `.nc` | `.yaml` metadata |
| `grep` | — | query file_path | stdout |

## Architecture

Dispatch flows through two levels:

1. **`src/lib.rs`** — `handle_dispatch()` parses `args[0]` as a module (`grep`, `netcdf`, `concat`) and delegates to the module's `run()` or sub-dispatcher.
2. **`src/netcdf.rs`** — `handle_target_dispatch()` parses `args[0]` as a format target (e.g., `nrt_ar`) and calls the corresponding submodule's `run()`.

Each converter submodule (e.g., `src/netcdf/nrt_ar.rs`) follows the same pattern:
- `run(args)` builds a `ConvertConfig` (src path, target path) and calls `netcdf_to_parquet()` or `netcdf_to_yaml()`
- The internal collection function opens the NetCDF, extracts variables using shared utilities from `common.rs`, assembles a Polars `DataFrame`, then writes Parquet via `ParquetWriter`

### Key shared utilities (`src/netcdf/common.rs`)
- `convert_time_value()` — converts days-since-1950-01-01 (standard oceanographic epoch) to Unix milliseconds
- `get_coordinate_value()` — reads a 1-D or scalar variable and tiles it to `time_len × obs_len`
- `get_var_float_value()` — reads float data, replacing fill values with NaN
- `get_qc_value()` — reads quality-control byte arrays
- `get_char_value()` / `get_char_value2()` / `get_char_vector3()` — read NetCDF `char` arrays (stored as `i8`) into `Vec<String>` with different dimension layouts
- `convert_depth_to_pressure()` / `convert_pressure_to_depth()` — bidirectional conversion using the `gsw` crate (TEOS-10 standard)

### Output DataFrame schema (parquet converters)
All converters produce a flat, observation-level table where NRT and CORA formats differ slightly:
- **NRT**: integer QC codes (`i8`), includes `pres_conv`/`deph_conv` flags indicating derived values
- **CORA**: character QC codes (`String`), uses `N_PROF × N_LEVELS` dimension layout

### `trace` feature
`#[cfg(feature = "trace")]` guards are available for debug logging. Enable with `cargo build --features trace`.
