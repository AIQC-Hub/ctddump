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
ctddump convert <subcommand> [OPTIONS] <src_file> <target_file>
```

| Subcommand | Input | Output | Config struct |
|------------|-------|--------|---------------|
| `nrt_ar` | NRT Arctic Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_bo` | NRT Baltic Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_mo` | NRT Mediterranean Sea `.nc` | `.parquet` | `NrtConfig` |
| `nrt_gl` | NRT Global `.nc` | `.parquet` | `NrtConfig` |
| `cora` | CORA `.nc` (current format) | `.parquet` | `CoraConfig` |
| `cora_legacy` | CORA `.nc` (older format) | `.parquet` | `CoraConfig` |

NRT subcommand flags: `--deph-source` / `--no-deph-source`, `--profile-coords` / `--no-profile-coords`

CORA subcommand flags: `--time-var <VAR>`, `--qc-type <int|char>`, `--time-qc` / `--no-time-qc`, `--deph-source` / `--no-deph-source`

### `batch` — directory-tree NetCDF → Parquet or YAML (multi-threaded)

```
ctddump batch <subcommand> [OPTIONS] <src_dir>
```

Recursively finds all `.nc` files under `<src_dir>` and processes them in parallel.
Output filenames keep the source stem and replace the extension with `.parquet` or `.yaml`.
If `--output` is omitted, each file is written alongside its source.
If `--output` is given, all files land flat in that directory — an error is raised before
conversion starts if any two sources would produce the same output filename.

**`batch convert`** — NetCDF → Parquet (same per-format flags as `convert`, plus `--output`, `--threads`):

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

### Config files and CLI overrides

All `convert` and `batch convert` subcommands support individual CLI flags for every
configuration field. The priority order is:

```
built-in default < --config file < individual CLI flags
```

`--config` / `-c` accepts a TOML file that sets any subset of fields (useful for saved presets).
Individual flags override whatever `--config` sets for that field only.

**NRT flags** (all `nrt_*` subcommands):

| Flag | Config field | Default by region |
|------|-------------|-------------------|
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` for BO/GL, `false` for AR/MO |
| `--profile-coords` / `--no-profile-coords` | `has_profile_coords` | `true` for BO, `false` for AR/MO/GL |

**CORA flags** (`cora` and `cora_legacy` subcommands):

| Flag | Config field | `cora` default | `cora_legacy` default |
|------|-------------|---------------|----------------------|
| `--time-var <VAR>` | `time_var` | `"TIME"` | `"JULD"` |
| `--qc-type <int\|char>` | `qc_type` | `int` | `char` |
| `--time-qc` / `--no-time-qc` | `has_time_qc` | `true` | `false` |
| `--deph-source` / `--no-deph-source` | `has_deph_source` | `true` | `false` |

**TOML config file format** (for `--config`):

```toml
# NRT
has_deph_source = true
has_profile_coords = false

# CORA
time_var = "TIME"
qc_type = "int"       # "int" or "char"
has_time_qc = true
has_deph_source = true
```

## Architecture

Dispatch is handled by `clap` in two stages:

1. **`src/cli.rs`** — defines the full CLI structure (`Cli`, `Commands`, `ConvertFormat`, `BatchConvertFormat`, `BatchHeaderFormat`, `HeaderFormat`) plus `NrtArgs` and `CoraArgs` flattened arg structs. Each arg struct carries per-field override flags and an `apply_to(&mut Config)` method.
2. **`src/lib.rs`** — `run(cli)` dispatches to `dispatch_convert()`, `dispatch_batch()`, or `dispatch_header()`. Each arm loads the TOML config (or built-in default via `load_or_default()`), then calls `nrt_args.apply_to()` / `cora_args.apply_to()` to layer CLI flag overrides on top.

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
- `get_qc_value()` — reads QC flags stored as `i8`, returns `Vec<String>` ("0"–"9"; `""` for missing)
- `get_qc_coordinate_value()` — like `get_qc_value` but tiles a coordinate-dimension variable (e.g., `TIME_QC`)
- `get_qc_value_from_char()` — reads QC flags stored as ASCII chars, returns `Vec<String>` (char as-is; `""` for space/null)
- `get_char_value()` / `get_char_value2()` / `get_char_vector3()` — read NetCDF `char` arrays (stored as `i8`) into `Vec<String>` with different dimension layouts
- `convert_depth_to_pressure()` / `convert_pressure_to_depth()` — bidirectional conversion using the `gsw` crate (TEOS-10 standard)

### Output DataFrame schema
All converters produce a uniform, observation-level flat table:

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
| `longitude` / `latitude` | `f32` or `f64` | NRT: f32, CORA: f64; NaN-filled from profile coords |
| `profile_longitude` / `profile_latitude` | `f32` (NRT) / `f64` (CORA) | NRT: from `PRECISE_*` or expanded `DEPLOY_*`; NaN when `has_profile_coords = false`. CORA: always NaN (no profile source). |
| `time_qc` / `position_qc` | `String` | `""` if absent in source |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `String` | single-char flag (e.g., `"1"`, `"A"`); `""` if missing |
| `pres_conv`, `deph_conv` | `i8` | `1` = value was derived by conversion |

### `trace` feature
`#[cfg(feature = "trace")]` guards are available for debug logging. Enable with `cargo build --features trace`.
