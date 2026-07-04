# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project Overview

`ctddump` is a Rust CLI that converts oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF to Parquet (data) or YAML (metadata). Two source repositories:
- **NRT** (Near Real Time): regional data — Arctic (AR), Baltic (BO), Mediterranean (MO), Global (GL).
- **CORA** (Copernicus Ocean Reanalysis): historical re-processed profiles (`cora` current format, `cora_legacy` older files).

Top-level commands: `convert` (single file → Parquet), `batch` (directory tree → Parquet/YAML, multi-threaded), `header` (NetCDF → YAML metadata), `concat` (merge Parquet files).

## Git Workflow

Permanent branches: `main` (stable releases), `develop` (integration). Commit day-to-day work directly to `develop`. `git-flow` (AVH Edition) is installed for multi-session features (`feature start/finish`), releases (`release …`), and hotfixes (`hotfix …`) — use it only when it adds value.

## System Dependencies

The `netcdf` crate links HDF5 via `hdf5-metno-sys`. Install dev headers first:
- Ubuntu/Debian: `sudo apt-get install libhdf5-dev libnetcdf-dev`
- macOS: `brew install hdf5`

> **Note:** HDF5 1.10.10 (on the CI runner) emits harmless `HDF5-DIAG` messages when reading files with the 1.12 attribute `_QuantizeBitGroomNumberOfSignificantDigits`. Data reads correctly; tests are unaffected.

## Commands

```bash
cargo build                        # build
cargo build --features trace       # build with debug-logging feature
cargo test                         # run all tests
cargo test test_convert_nrt_ar_1   # run one test by name
cargo run -- <cmd> [subcmd] --help # discover any command's interface
```

The full CLI (commands, subcommands, flags, defaults, TOML config format) is defined with `clap` in `src/cli.rs` and is self-documenting via `--help` at every level — consult that rather than mirroring it here.

### Test Data

Fixtures in `tests/` are not committed. Fetch them with `scripts/fetch_test_data.sh` (needs authenticated `gh` CLI and `unzip`; override with `TEST_DATA_VERSION` / `TEST_DATA_REPO`).

## CLI notes (non-obvious behavior)

- **Config priority:** `built-in default < --config <TOML> < individual CLI flags`. `--config`/`-c` sets any subset of fields; per-field flags override it for that field only.
- **NRT region defaults:** `has_deph_source` is `true` for BO/GL, `false` for AR/MO. `has_profile_coords` is `true` for BO only, `false` for AR/MO/GL.
- **CORA `cora` vs `cora_legacy` defaults:** `time_var` `TIME`/`JULD`; `qc_type` `int`/`char`; `has_time_qc` `true`/`false`; `has_deph_source` `true`/`false`.
- **`--pattern`** matches filenames only (not paths); supports `*`, `?`, `[…]`. Ignored by single-file `convert`.
- **`batch` output:** without `--output`, each result is written beside its source; with `--output`, all land flat in that dir and a duplicate-output-name collision is an error raised before conversion starts.

## Architecture

Two-stage `clap` dispatch:
1. **`src/cli.rs`** — full CLI structure plus flattened `NrtArgs` / `CoraArgs` arg structs. Each carries per-field override flags and an `apply_to(&mut Config)` method.
2. **`src/lib.rs`** — `run(cli)` dispatches to `dispatch_convert()` / `dispatch_batch()` / `dispatch_header()`. Each loads TOML config (or built-in default via `load_or_default()`), then layers CLI overrides via `*_args.apply_to()`.

**Converters** (`src/convert/nrt.rs`, `cora.rs`): each exposes `convert_file()` (shared with batch) and `run()` (single-file CLI entry). They open the NetCDF, extract variables via `common.rs`, assemble a Polars `DataFrame`, and write Parquet with `ParquetWriter`.

**Batch** (`src/batch.rs`): `collect_nc_files()` (walkdir), `compute_output_pairs()`, `check_duplicates()`, `run_batch()` (rayon parallel; optional thread count; output ext `"parquet"`/`"yaml"`).

> **Threading & stack (batch mode):** parallelism is per file. `run_batch` builds an explicit rayon pool with a 16 MiB worker stack — rayon's 2 MiB default overflows inside Polars' parquet writer on large files (single-file `convert` avoids this only because it runs on the main thread's 8 MiB stack). It also sets `POLARS_MAX_THREADS=1` so Polars' own global pool doesn't spawn N_cpus extra threads on top of `--threads`; without it `--threads N` yields ≈ N + N_cpus workers. `main` raises `RUST_MIN_STACK` to 16 MiB so Polars' pool threads get the larger stack too. All three respect a value the user sets in the environment.

**Header** (`src/header/{nrt,cora}.rs`, `common.rs`): metadata → YAML; shared `collect_dimensions()`, `collect_global_attributes()`, `collect_variables_and_metadata()`.

**Shared utilities** (`src/convert/common.rs`):
- `convert_time_value()` — days-since-1950-01-01 (oceanographic epoch) → Unix ms.
- `get_coordinate_value()` — reads 1-D/scalar var, tiles to `time_len × obs_len`; fill values if absent.
- `get_var_float_value()` — float data with fill → NaN.
- `get_qc_value()` / `get_qc_coordinate_value()` / `get_qc_value_from_char()` — QC flags (i8, tiled coordinate, or ASCII char) → `Vec<String>`.
- `get_char_value()` / `get_char_value2()` / `get_char_vector3()` — NetCDF `char` arrays (i8) → `Vec<String>` at different dim layouts.
- `convert_depth_to_pressure()` / `convert_pressure_to_depth()` — bidirectional via `gsw` (TEOS-10).

### Output DataFrame schema

Every converter produces a uniform observation-level flat table:

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
| `longitude` / `latitude` | `f32` (NRT) / `f64` (CORA) | NaN-filled from profile coords |
| `profile_longitude` / `profile_latitude` | `f32` (NRT) / `f64` (CORA) | NRT: `PRECISE_*` or expanded `DEPLOY_*`, NaN if `has_profile_coords = false`. CORA: always NaN. |
| `time_qc` / `position_qc` | `String` | `""` if absent |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `String` | single-char flag; `""` if missing |
| `pres_conv`, `deph_conv` | `i8` | `1` = value derived by conversion |

### `trace` feature

`#[cfg(feature = "trace")]` guards enable debug logging. Build with `cargo build --features trace`.
