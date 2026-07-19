# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project Overview

`ctddump` is a Rust CLI that converts oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF to Parquet (data) or YAML (metadata). Two source repositories:
- **NRT** (Near Real Time): regional data — Arctic (AR), Baltic (BO), Mediterranean (MO), Global (GL).
- **CORA** (Copernicus Ocean Reanalysis): historical re-processed profiles (`cora` current format, `cora_legacy` older files).

Top-level commands: `convert` (single file → Parquet), `batch` (directory tree → Parquet/YAML, multi-threaded), `header` (NetCDF → YAML metadata), `concat` (merge Parquet files), `report` (summarise a Parquet/YAML file as text), `filter` (keep/drop profiles by a bounding box), `dropna` (drop profiles that are all-NA in any of temp/psal/pres), `dropqc` (drop profiles flagged bad in time_qc/position_qc), `markdup` (mark duplicate profiles with an is_dup column), `dedup` (remove duplicate profiles), `compare` (two-way coverage summary for two Parquet files).

## Documentation style

Do not use em dashes (`—`) in any human-facing document: `README.md`, the mdBook docs under `docs/`, `CHANGELOG.md`, the generated reports and summary pages, and the summary web site. This covers both prose committed to the repo and text the tools emit at runtime (report/summary renderers, script log lines, help text). Use a colon, comma, parentheses, semicolon, or a reworded sentence instead.

## Git Workflow

Permanent branches: `main` (stable releases), `develop` (integration). Commit day-to-day work directly to `develop`. `git-flow` (AVH Edition) is installed for multi-session features (`feature start/finish`) and hotfixes (`hotfix …`) — use it only when it adds value.

Releases are cut by merging `develop` into `main` and tagging `vX.Y.Z` (not via `git flow release`). See [`RELEASING.md`](RELEASING.md) for the full procedure — version bump, `Cargo.lock` sync, `CHANGELOG.md`, merge, tag, push.

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
- **Empty matches are a no-op, not an error:** `batch` (convert/header) and `concat` (convert/header) print an informational message to stderr and write no output when the pattern matches no files or the input dir is empty/absent (`collect_nc_files`/`collect_parquet_files` return an empty `Vec`; the `run_*` callers early-return `Ok`). This lets a workflow reference a not-yet-available dataset (e.g. Baltic GL) without failing. The helper scripts (`convert_data.sh`/`clean_data.sh`/`dedup_data.sh`) additionally skip a per-file step whose input file is missing.
- **`batch` output:** without `--output`, each result is written beside its source; with `--output`, all land flat in that dir and a duplicate-output-name collision is an error raised before conversion starts.
- **`concat convert` renumber sort:** by default rows are sorted by `platform_code, profile_timestamp, longitude, latitude, pres` before `profile_no`/`observation_no` are assigned. `--no-pres-sort` drops the `pres` key and makes the sort stable, so observations keep their original per-profile source order (`observation_no` follows acquisition order instead of ascending pressure). Ignored with `--no-renumber`.
- **`concat convert` missing-pres dropping (default on):** rows whose `pres` is null/NaN are dropped before merging. In the renumber path the filter runs before numbering so `observation_no` stays contiguous (`1..N`) over the surviving rows; it is also honored under `--no-renumber` as a plain row filter. Pass `--keep-na-pres` to retain those rows.

## Architecture

Two-stage `clap` dispatch:
1. **`src/cli.rs`** — full CLI structure plus flattened `NrtArgs` / `CoraArgs` arg structs. Each carries per-field override flags and an `apply_to(&mut Config)` method.
2. **`src/lib.rs`** — `run(cli)` dispatches to `dispatch_convert()` / `dispatch_batch()` / `dispatch_header()`. Each loads TOML config (or built-in default via `load_or_default()`), then layers CLI overrides via `*_args.apply_to()`.

**Converters** (`src/convert/nrt.rs`, `cora.rs`): each exposes `convert_file()` (shared with batch) and `run()` (single-file CLI entry). They open the NetCDF, extract variables via `common.rs`, assemble a Polars `DataFrame`, and write Parquet with `ParquetWriter`.

> **Streaming (memory).** Converters never materialize the full dense `TIME × DEPTH` (`N_PROF × N_LEVELS`) grid. `netcdf_to_parquet` opens the file once, walks `common::time_chunks()` slices of the outer dimension, and for each chunk assembles → writes one Parquet **row group** via Polars' `BatchedWriter` (which flushes to disk per `write_batch`; `finish` writes only the footer). A zero-row chunk defines the schema up front so it matches the real chunks. The `common::get_*_chunk` readers slice the NetCDF (`[t0..t1, 0..obs_len]`); rank-0 scalars are read with the `..` full-extent selector (an explicit `[0..1]` range fails on them). Profile numbering and DEPLOY-coordinate expansion are anchored to **absolute** outer indices, so a chunk yields exactly what a whole-file pass would — output row values/order are chunk-size-independent (only the row-group layout changes). Chunk target size is `common::chunk_rows()` (default 1,000,000 obs rows), overridable via `CTDDUMP_CHUNK_ROWS` (lower = less memory, higher = fewer row groups). On a 230 MB / 15 M-cell file this cut peak RSS from ~8.8 GB to ~0.7 GB (default) / ~0.1 GB (small chunk).

> **Polars parallel-op memory leak (why `retain_by_mask` + `set_parallel(false)`).** Polars 0.43.1 leaks memory on every *parallel column operation*: `DataFrame::filter`/`take` (parallel row-gather over the string columns) and `ParquetWriter`'s default parallel column encoding each retain memory per call that is never freed — independent of the allocator (glibc and mimalloc behave identically) and not reclaimable by `malloc_trim`/`H5garbage_collect`. It's invisible on a single file but in `batch` mode it accumulates ~0.2 MB per file **without bound** (7905 tiny CORA files → 88 GB RSS, climbing to the end). Two avoidances, both in the converters: (1) `collect_*_chunk` drops all-NaN observation rows on the **raw `Vec`s** via `common::retain_by_mask` *before* building the DataFrame, so no Polars `filter`/`take` is called (and the dense all-NaN levels never enter Polars); (2) every `ParquetWriter` in the converters **and** `concat` uses `.set_parallel(false)`. This bounds `batch convert` RSS to a small constant regardless of file count (3000 small CORA files: ~0.9 GB → ~40 MB single-thread) and is verified data-identical to the old parallel path. If Polars is upgraded to a release that fixes these leaks, both workarounds can be reconsidered.

> **Polars slice-pushdown bug on multi-row-group Parquet (why `common::seq_scan_args()`).** Polars 0.43.1's *parallel* Parquet reader (`ParallelStrategy::Auto`/`RowGroups`/`Prefiltered`, the default) mishandles `slice` pushdown across row groups: `scan_parquet(default).slice(offset, n)` returns rows from the **first row group** for *every* offset. This silently corrupts the chunked streaming passes in `filter`/`dropqc`/`dropna`/`markdup`/`dedup` (each does `scan().slice(offset, count)` per `chunk_rows()` window) whenever the input has **more than one row group** — i.e. any file over `chunk_rows()` rows, including every multi-row-group `concat`/`markdup` output. It is invisible in unit tests because in-test fixtures written with `ParquetWriter::finish` are a single row group. Fix: those five modules scan with `common::seq_scan_args()` (`ScanArgsParquet { parallel: ParallelStrategy::None, .. }`), which slices correctly. Whole-file `group_by`/`collect` (e.g. `report`, `concat`'s predicate-filter reads) is **not** affected and keeps the default parallel reader. Regression-guarded by `tests/test_dedup_streaming.rs` (markdup writes a many-row-group file which dedup then slices). If Polars fixes the pushdown, `seq_scan_args` can revert to `ScanArgsParquet::default()`.

**Batch** (`src/batch.rs`): `collect_nc_files()` (walkdir), `compute_output_pairs()`, `check_duplicates()`, `run_batch()` (rayon parallel; optional thread count; output ext `"parquet"`/`"yaml"`).

> **Threading & stack (batch mode):** parallelism is per file. `run_batch` builds an explicit rayon pool with a 16 MiB worker stack — rayon's 2 MiB default overflows inside Polars' parquet writer on large files (single-file `convert` avoids this only because it runs on the main thread's 8 MiB stack). It also sets `POLARS_MAX_THREADS=1` so Polars' own global pool doesn't spawn N_cpus extra threads on top of `--threads`; without it `--threads N` yields ≈ N + N_cpus workers. `main` raises `RUST_MIN_STACK` to 16 MiB so Polars' pool threads get the larger stack too. All three respect a value the user sets in the environment.

**Header** (`src/header/{nrt,cora}.rs`, `common.rs`): metadata → YAML; shared `collect_dimensions()`, `collect_global_attributes()`, `collect_variables_and_metadata()`.

**Concat** (`src/concat.rs`): `run_concat_parquet()` merges Parquet files; `run_concat_header()` merges YAML. `renumber()` reassigns `profile_no` (dense rank of the `platform_code|timestamp|lon|lat` key, `.over(["platform_code"])`) and `observation_no` (`.over(["platform_code","profile_no"])`).

> **Streaming (memory).** `run_concat_parquet` never loads all inputs at once. Because `renumber` partitions by `platform_code`, the merge is done one **contiguous `platform_code` range** at a time and each range is streamed out as a Parquet **row group** via `BatchedWriter`. Pass 1 (`scan_platform_index`) reads only the `platform_code` column of every file to get per-platform row counts and per-file min/max platform; `partition_platform_ranges` groups platforms into ranges of at most `common::chunk_rows()` obs rows (`CTDDUMP_CHUNK_ROWS`). Pass 2 (`build_range_df`) assembles each range by re-scanning only the overlapping files with a `platform_code` filter (predicate pushdown skips non-matching row groups), runs the **same** `renumber`, and writes. Emitting ranges in ascending order is data-identical to a whole-dataset renumber — only the row-group layout changes (verified by `tests/test_concat_streaming.rs`). This also preserves cross-file profile merging (a profile split across files shares a `platform_code`, so it lands in one range). `--no-renumber` skips the two passes and streams each file straight through as its own row group. NRT files (one platform each) are read once total; multi-platform files (e.g. CORA) are re-scanned once per overlapping range.

> **Threading (`--threads`/`-t`).** Because each platform range owns *complete* platforms, ranges are independent units of parallel work (this is why concat parallelizes by range, not by file — a platform can span files and a file can hold many platforms). The effective worker count is `--threads N` or, when omitted, all logical CPU cores. When it is `> 1` the renumber path builds a rayon pool (16 MiB worker stacks, as in `batch`) and renumbers ranges concurrently, each writing a temp Parquet file beside the output named `<output>.concat-tmp-NNNNN` (no `.parquet` suffix, so a stray temp from a crash is never re-globbed). The temp files are then concatenated in range order into the final output and removed, so the result is byte-identical to the sequential path (verified by `tests/test_concat_parallel.rs`). It sets `POLARS_MAX_THREADS=1` **before any Polars call** (Polars reads it once at pool init) so `--threads` is the real knob and each range worker doesn't spawn Polars' own N_cpus pool on top. Peak memory rises to ≈ `threads × CTDDUMP_CHUNK_ROWS` rows plus temp disk — so good range-level parallelism wants `#ranges ≥ threads` (lower `CTDDUMP_CHUNK_ROWS` for more, smaller ranges). `--threads 1` forces the sequential single-writer path (lowest memory; Polars still parallelizes that one stream); `--no-renumber` ignores `--threads`.

**Report** (`src/report/{parquet,yaml,format}.rs`): summarise a produced Parquet data file (`report parquet --level global|platform|profile`) or a merged header YAML (`report yaml`) as TSV/text/JSON. Aggregates via Polars `LazyFrame`; `format.rs` hand-rolls the three output writers. When the scanned file has a `markdup` `is_dup` column, the parquet report adds duplicate counts (`dup_profiles` at global/platform, the `is_dup` flag at profile level); absent the column the report is unchanged.

**Filter** (`src/filter.rs`): `run()` keeps (`--mode include`, default) or drops (`--mode exclude`) whole profiles by a geographic bounding box. Since `longitude`/`latitude` are constant within a profile, it is a plain per-row predicate (NaN positions are treated as outside via `is_not_nan` guards). Streamed in `common::chunk_rows()` row slices via `BatchedWriter` (`set_parallel(false)`), so peak memory is bounded regardless of file size.

**Dropna** (`src/dropna.rs`): `run()` drops whole profiles that are entirely NA in any of `temp`/`psal`/`pres` (kept iff each parameter has ≥1 valid observation). Two streaming passes bound memory: `build_keep_set` OR-s each chunk's per-`(platform_code, profile_no)` "has ≥1 valid" flags into a `HashMap` (a hash group-by, so it is correct even when a profile's rows straddle a chunk boundary); pass 2 re-streams the slices and writes only rows whose profile is in the keep-set (`keep_mask` + `filter`, `BatchedWriter` `set_parallel(false)`). Verified by `tests/test_dropna.rs`, including cross-chunk merge.

**Dropqc** (`src/dropqc.rs`): `run()` drops whole profiles whose profile-level QC is bad — a profile is kept iff **both** `time_qc` and `position_qc` are `"1"` (OK) or `""` (missing / NA). Missing is kept on purpose (many files ship no profile-level QC; a `-128` NA byte and an absent variable both map to `""` via `common::i8_to_qc_string`, so the empty-string check covers both). Note `"9"` (Argo "missing value" flag) is a *present* value, not the NA byte, so it is dropped. Since these QC columns are constant within a profile, it is a plain per-row predicate (`(qc == "1") | (qc == "")` for each), streamed one `chunk_rows()` slice at a time via `BatchedWriter` (`set_parallel(false)`) — the same shape as **Filter**, no group-by needed. Verified by `tests/test_dropqc.rs`.

**Dupkey** (`src/dupkey.rs`): shared duplicate-key logic for markdup/dedup. `KeyOpts { time_format, decimals, round_mode }` (defaults: `%Y-%m-%d` date-only, 3 decimals, round-to-nearest) with `key(ts_ms, lon, lat) -> Option<DupKey>` where `DupKey = (formatted_time, lon_scaled_i64, lat_scaled_i64)`. `platform_code` is **not** in the key (duplicates are cross-platform); NaN position or null timestamp → `None` (never a duplicate). `RoundMode` is a `clap::ValueEnum` used directly by the CLI.

**Markdup** (`src/markdup.rs`): `run()` adds a Boolean `is_dup` column and writes a duplicates TSV. Two streaming passes: pass 1 reduces the file to one `ProfileRec` per `(platform_code, profile_no)` (key, n_obs, first-row details), counts distinct profiles per key, assigns a sorted `dup_group` id to each key with count > 1, and writes the TSV (`dup_group, platform_code, profile_no, profile_time, profile_timestamp, longitude, latitude, n_obs`); pass 2 re-streams and appends `is_dup` (true iff the profile's key is shared). Verified by `tests/test_dedup.rs`.

**Dedup** (`src/dedup.rs`): `run()` removes duplicate profiles. Pass 1 builds per-profile `(key, n_obs, order)` and picks the winner of each key (max `n_obs`, ties → lowest first-seen `order`); keep-set = winners ∪ all no-key profiles. Pass 2 re-streams, keeps only keep-set rows (`keep_mask` + `filter`), and resets `is_dup` to `false` if the column is present (survivors are unique — keeps the `markdup → dedup` schema stable). Re-derives the key itself (same `KeyOpts`) so it runs standalone. Verified by `tests/test_dedup.rs` and `tests/test_dedup_streaming.rs`.

**Compare** (`src/compare.rs`): `run()` reports two-way coverage between two Parquet files. Reuses `dupkey::KeyOpts` for the time/position key but **prepends `platform_code`** by default (`--no-platform-key` drops it) — the opposite default to markdup/dedup, because two compared files are usually extracts of the same platforms. Each file is streamed in `chunk_rows()` slices and reduced to one `(key, n_obs)` per `(platform, profile_no)` (same shape as markdup pass 1), so memory follows profile count, not file size. The other file is indexed as `HashMap<CmpKey, HashSet<u64>>` (a *set* of observation counts, since several profiles can share a key; counts agree if **any** carrier matches). Emits one row per direction via `report::format::write_report`, second file as reference first. `--time-col` accepts either dtype: `Datetime` is read as ms, `Float64`/`Float32` as days since 1950 (`EPOCH_1950_MS`, non-finite → no key). Percentages with a zero denominator are NaN, which the formatters render as empty rather than a misleading `0`. Verified by `tests/test_compare.rs` and `tests/test_compare_streaming.rs` (the latter is its own binary because `CTDDUMP_CHUNK_ROWS` is process-global).

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
