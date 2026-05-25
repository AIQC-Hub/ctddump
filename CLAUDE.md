# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`ctddump` is a Rust CLI tool for converting oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF format to Parquet (data) or YAML (metadata). It targets data from two oceanographic repositories:
- **NRT** (Near Real Time): data from various regions — Arctic Sea (AR), Baltic Sea (BO), Mediterranean Sea (MO), and Global (GL)
- **CORA** (Copernicus Ocean Reanalysis): historical re-processed CTD profiles (`cora` for current format, `cora_legacy` for older files)

The CLI exposes four top-level commands: `convert` (single-file NetCDF → Parquet), `batch` (directory-tree NetCDF → Parquet with multi-threading), `header` (NetCDF → YAML metadata), and `concat` (future).

## Git Workflow

Two permanent branches: `main` (stable releases) and `develop` (integration). `git-flow` AVH Edition is installed but only used when it adds real value.

| Situation | Approach |
|-----------|----------|
| Normal day-to-day changes | Commit directly to `develop` |
| Large feature spanning multiple sessions | `git flow feature start/finish <name>` |
| Cutting a release | `git flow release start/finish <version>` |
| Urgent fix to `main` | `git flow hotfix start/finish <name>` |

## Commands

```bash
# Build
cargo build

# Run all tests
cargo test

# Run a single test by name
cargo test test_convert_nrt_ar_1

# Build with trace feature enabled
cargo build --features trace

# Run the binary
cargo run -- <command> [subcommand] [options] <src_file> <target_file>
# e.g.:
cargo run -- convert nrt_ar input.nc output.parquet
cargo run -- convert nrt_bo --config custom.toml input.nc output.parquet
cargo run -- convert nrt_head input.nc output.yaml
cargo run -- --help
cargo run -- convert --help
cargo run -- convert nrt_ar --help
```

### Test Data

Tests require fixture files in `tests/`. These are not committed to the repo — download them via:

```bash
scripts/fetch_test_data.sh
# Requires: gh CLI (authenticated) and unzip
# Optionally override: TEST_DATA_VERSION=v0.1.0 TEST_DATA_REPO=AIQC-Hub/ctddump
```

## CLI Interface

Built with `clap` — every command and subcommand supports `-h`/`--help`.

### `convert` — NetCDF → Parquet

```
ctddump convert <subcommand> [--config <file.toml>] <src_file> <target_file>
```

| Subcommand | Input | Output | Config struct |
|------------|-------|--------|---------------|
| `nrt_ar` | NRT Arctic Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_bo` | NRT Baltic Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_mo` | NRT Mediterranean Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_gl` | NRT Global `.nc` | `.parquet` | `NrtConfig` |
| `cora` | CORA `.nc` (current format) | `.parquet` | `CoraConfig` |
| `cora_legacy` | CORA `.nc` (older format) | `.parquet` | `CoraConfig` |

### `batch` — directory-tree NetCDF → Parquet or YAML (multi-threaded)

```
ctddump batch <subcommand> [--config <file.toml>] [--output <dest_dir>] [--threads <n>] <src_dir>
```

Recursively finds all `.nc` files under `<src_dir>` and processes them in parallel.
Output filenames keep the source stem and replace the extension with `.parquet` or `.yaml`.
If `--output` is omitted, each file is written alongside its source.
If `--output` is given, all files land flat in that directory — an error is raised before
conversion starts if any two sources would produce the same output filename.

**`batch convert`** — NetCDF → Parquet:

| Subcommand | Format |
|------------|--------|
| `nrt_ar` | NRT Arctic Sea |
| `nrt_bo` | NRT Baltic Sea |
| `nrt_mo` | NRT Mediterranean Sea |
| `nrt_gl` | NRT Global |
| `cora` | CORA current format |
| `cora_legacy` | CORA legacy format |

**`batch header`** — NetCDF → YAML metadata:

| Subcommand | Format |
|------------|--------|
| `nrt` | Any NRT `.nc` |
| `cora` | Any CORA `.nc` |

### `header` — NetCDF → YAML metadata

```
ctddump header <subcommand> <src_file> <target_file>
```

| Subcommand | Input | Output |
|------------|-------|--------|
| `nrt` | Any NRT `.nc` | `.yaml` metadata |
| `cora` | Any CORA `.nc` | `.yaml` metadata |

### Config files

Each NRT and CORA subcommand accepts an optional `--config` / `-c` TOML file to override the embedded defaults. Omitting `--config` uses the embedded default for that subcommand.

**NRT config** (`src/convert/nrt_config.rs`):
```toml
has_deph_source = true      # whether DEPH variable exists in the source file
has_precise_coords = false  # whether to use PRECISE_LONGITUDE/PRECISE_LATITUDE
```

**CORA config** (`src/convert/cora_config.rs`):
```toml
time_var = "TIME"    # time variable name ("TIME" or "JULD" for legacy)
qc_type = "int"      # QC storage type: "int" (i8) or "char" (converted to i8)
has_time_qc = true   # whether TIME_QC / POSITION_QC variables exist
has_deph_source = true  # whether DEPH variable exists in the source file
```

## Architecture

Dispatch is handled by `clap` in two stages:

1. **`src/cli.rs`** — defines the full CLI structure (`Cli`, `Commands`, `ConvertFormat`, `BatchFormat`, `HeaderFormat`) using clap derive macros.
2. **`src/lib.rs`** — `run(cli)` dispatches to `dispatch_convert()`, `dispatch_batch()`, or `dispatch_header()`, loads the TOML config (or falls back to the embedded default via `load_or_default()`), and calls the appropriate module.

### Convert modules (`src/convert/`)
Each converter follows the same pattern:
- `run(args, config, target)` builds a `ConvertConfig` (src/dest paths) and calls `netcdf_to_parquet()`
- The internal collection function opens the NetCDF, extracts variables using shared utilities from `common.rs`, assembles a Polars `DataFrame`, and writes Parquet via `ParquetWriter`

- **`src/convert/nrt.rs`** — unified NRT converter; exposes `convert_file()` (used by both `run` and batch) and `run()` (single-file CLI entry point)
- **`src/convert/cora.rs`** — unified CORA converter; same structure as `nrt.rs`

### Batch module (`src/batch.rs`)
- `collect_nc_files()` — recursively walk a directory for `.nc` files (`walkdir`)
- `compute_output_pairs()` — derive flat or in-place output paths
- `check_duplicates()` — pre-flight duplicate detection
- `run_batch()` — parallel execution via `rayon`; accepts an optional thread count and output extension (`"parquet"` or `"yaml"`)

### Header modules (`src/header/`)
- **`src/header/nrt.rs`** — NRT metadata extraction to YAML
- **`src/header/cora.rs`** — CORA metadata extraction to YAML
- **`src/header/common.rs`** — shared utilities: `collect_dimensions()`, `collect_global_attributes()`, `collect_variables_and_metadata()`

### Key shared utilities (`src/convert/common.rs`)
- `convert_time_value()` — converts days-since-1950-01-01 (standard oceanographic epoch) to Unix milliseconds
- `get_coordinate_value()` — reads a 1-D or scalar variable and tiles it to `time_len × obs_len`; returns fill values if the variable is absent
- `get_var_float_value()` — reads float data, replacing fill values with NaN
- `get_qc_value()` — reads QC flags stored as `i8`
- `get_qc_value_from_char()` — reads QC flags stored as ASCII digit chars and converts to `i8`
- `get_char_value()` / `get_char_value2()` / `get_char_vector3()` — read NetCDF `char` arrays (stored as `i8`) into `Vec<String>` with different dimension layouts
- `convert_depth_to_pressure()` / `convert_pressure_to_depth()` — bidirectional conversion using the `gsw` crate (TEOS-10 standard)

### Output DataFrame schema
All converters produce a uniform, observation-level flat table with integer QC codes (`i8`):

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
| `longitude` / `latitude` | `f32` or `f64` | NRT: f32, CORA: f64 |
| `time_qc` / `position_qc` | `i8` | filled with `i8::MIN` if absent in source |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `i8` | |
| `pres_conv`, `deph_conv` | `i8` | `1` = value was derived by conversion |

### `trace` feature
`#[cfg(feature = "trace")]` guards are available for debug logging. Enable with `cargo build --features trace`.
