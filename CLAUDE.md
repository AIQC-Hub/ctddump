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
- **NRT region defaults:** `has_deph_source` is `true` for BO/GL, `false` for AR/MO. `has_profile_coords` is `true` for BO only, `false` for AR/MO/GL. **DEPH is auto-detected:** NRT always reads DEPH and does the bidirectional PRES↔DEPH conversion when the file contains a `DEPH` variable, regardless of `has_deph_source` — the flag only forces DEPH handling on for files where the variable might otherwise be skipped. This matters for AR/MO files that ship DEPH instead of PRES (without it both `pres` and `deph` would be all-NaN). When DEPH is absent the sourced/derived branches are equivalent, so PRES-only output is unchanged. CORA does **not** auto-detect (its `cora_legacy` default deliberately ignores a present DEPH).
- **CORA `cora` vs `cora_legacy` defaults:** `time_var` `TIME`/`JULD`; `qc_type` `int`/`char`; `has_time_qc` `true`/`false`; `has_deph_source` `true`/`false`.
- **`--pattern`** matches filenames only (not paths); supports `*`, `?`, `[…]`. Ignored by single-file `convert`.
- **`batch` output:** without `--output`, each result is written beside its source; with `--output`, all land flat in that dir and a duplicate-output-name collision is an error raised before conversion starts.
- **`concat convert` renumber sort:** by default rows are sorted by `platform_code, profile_timestamp, longitude, latitude, pres` before `profile_no`/`observation_no` are assigned. `--no-pres-sort` drops the `pres` key and makes the sort stable, so observations keep their original per-profile source order (`observation_no` follows acquisition order instead of ascending pressure). Ignored with `--no-renumber`.
- **`concat convert` missing-pres dropping (default on):** rows whose `pres` is null/NaN are dropped before merging. In the renumber path the filter runs before numbering so `observation_no` stays contiguous (`1..N`) over the surviving rows; it is also honored under `--no-renumber` as a plain row filter. Pass `--keep-na-pres` to retain those rows.

## Architecture

Two-stage `clap` dispatch:
1. **`src/cli.rs`** — full CLI structure plus flattened `NrtArgs` / `CoraArgs` arg structs. Each carries per-field override flags and an `apply_to(&mut Config)` method.
2. **`src/lib.rs`** — `run(cli)` dispatches to `dispatch_convert()` / `dispatch_batch()` / `dispatch_header()`. Each loads TOML config (or built-in default via `load_or_default()`), then layers CLI overrides via `*_args.apply_to()`.

**Converters** (`src/convert/nrt.rs`, `cora.rs`): each exposes `convert_file()` (shared with batch) and `run()` (single-file CLI entry). They open the NetCDF, extract variables via `common.rs`, assemble a Polars `DataFrame`, and write Parquet with `ParquetWriter`.

> **Streaming (memory).** Converters never materialize the full dense `TIME × DEPTH` (`N_PROF × N_LEVELS`) grid. `netcdf_to_parquet` opens the file once, walks `common::time_chunks()` slices of the outer dimension, and for each chunk assembles → filters → writes one Parquet **row group** via Polars' `BatchedWriter` (which flushes to disk per `write_batch`; `finish` writes only the footer). A zero-row chunk defines the schema up front so it matches the real chunks. The `common::get_*_chunk` readers slice the NetCDF (`[t0..t1, 0..obs_len]`); rank-0 scalars are read with the `..` full-extent selector (an explicit `[0..1]` range fails on them). Profile numbering and DEPLOY-coordinate expansion are anchored to **absolute** outer indices, so a chunk yields exactly what a whole-file pass would — output row values/order are chunk-size-independent (only the row-group layout changes). Chunk target size is `common::chunk_rows()` (default 1,000,000 obs rows), overridable via `CTDDUMP_CHUNK_ROWS` (lower = less memory, higher = fewer row groups). On a 230 MB / 15 M-cell file this cut peak RSS from ~8.8 GB to ~0.7 GB (default) / ~0.1 GB (small chunk).

**Batch** (`src/batch.rs`): `collect_nc_files()` (walkdir), `compute_output_pairs()`, `check_duplicates()`, `run_batch()` (rayon parallel; optional thread count; output ext `"parquet"`/`"yaml"`).

> **Threading & stack (batch mode):** parallelism is per file. `run_batch` builds an explicit rayon pool with a 16 MiB worker stack — rayon's 2 MiB default overflows inside Polars' parquet writer on large files (single-file `convert` avoids this only because it runs on the main thread's 8 MiB stack). It also sets `POLARS_MAX_THREADS=1` so Polars' own global pool doesn't spawn N_cpus extra threads on top of `--threads`; without it `--threads N` yields ≈ N + N_cpus workers. `main` raises `RUST_MIN_STACK` to 16 MiB so Polars' pool threads get the larger stack too. All three respect a value the user sets in the environment.

**Header** (`src/header/{nrt,cora}.rs`, `common.rs`): metadata → YAML; shared `collect_dimensions()`, `collect_global_attributes()`, `collect_variables_and_metadata()`.

**Concat** (`src/concat.rs`): `run_concat_parquet()` merges Parquet files; `run_concat_header()` merges YAML. `renumber()` reassigns `profile_no` (dense rank of the `platform_code|timestamp|lon|lat` key, `.over(["platform_code"])`) and `observation_no` (`.over(["platform_code","profile_no"])`).

> **Streaming (memory).** `run_concat_parquet` never loads all inputs at once. Because `renumber` partitions by `platform_code`, the merge is done one **contiguous `platform_code` range** at a time and each range is streamed out as a Parquet **row group** via `BatchedWriter`. Pass 1 (`scan_platform_index`) reads only the `platform_code` column of every file to get per-platform row counts and per-file min/max platform; `partition_platform_ranges` groups platforms into ranges of at most `common::chunk_rows()` obs rows (`CTDDUMP_CHUNK_ROWS`). Pass 2 (`build_range_df`) assembles each range by re-scanning only the overlapping files with a `platform_code` filter (predicate pushdown skips non-matching row groups), runs the **same** `renumber`, and writes. Emitting ranges in ascending order is data-identical to a whole-dataset renumber — only the row-group layout changes (verified by `tests/test_concat_streaming.rs`). This also preserves cross-file profile merging (a profile split across files shares a `platform_code`, so it lands in one range). `--no-renumber` skips the two passes and streams each file straight through as its own row group. NRT files (one platform each) are read once total; multi-platform files (e.g. CORA) are re-scanned once per overlapping range.

> **Threading (`--threads`/`-t`).** Because each platform range owns *complete* platforms, ranges are independent units of parallel work (this is why concat parallelizes by range, not by file — a platform can span files and a file can hold many platforms). The effective worker count is `--threads N` or, when omitted, all logical CPU cores. When it is `> 1` the renumber path builds a rayon pool (16 MiB worker stacks, as in `batch`) and renumbers ranges concurrently, each writing a temp Parquet file beside the output named `<output>.concat-tmp-NNNNN` (no `.parquet` suffix, so a stray temp from a crash is never re-globbed). The temp files are then concatenated in range order into the final output and removed, so the result is byte-identical to the sequential path (verified by `tests/test_concat_parallel.rs`). It sets `POLARS_MAX_THREADS=1` **before any Polars call** (Polars reads it once at pool init) so `--threads` is the real knob and each range worker doesn't spawn Polars' own N_cpus pool on top. Peak memory rises to ≈ `threads × CTDDUMP_CHUNK_ROWS` rows plus temp disk — so good range-level parallelism wants `#ranges ≥ threads` (lower `CTDDUMP_CHUNK_ROWS` for more, smaller ranges). `--threads 1` forces the sequential single-writer path (lowest memory; Polars still parallelizes that one stream); `--no-renumber` ignores `--threads`.

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
