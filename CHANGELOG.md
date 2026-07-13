# Changelog

## [Unreleased]

## [0.12.2] - 2026-07-13

### Changed
- `batch convert`/`batch header` schedule files **largest-first** and dispatch **one file per work-stealing unit** (`with_max_len(1)`), improving multi-threaded throughput on file sets with mixed sizes. Rayon weights every file equally and cannot split a single file across threads, so previously a large file scheduled late left the other workers idle waiting on it; starting the heavy files first lets the small ones backfill that tail. No change to output (files convert independently)

## [0.12.1] - 2026-07-13

### Fixed
- `markdup` no longer panics ("RecordBatch requires all its arrays to have an equal number of rows") and truncate its output on inputs with more than one Parquet row group (any file over `CTDDUMP_CHUNK_ROWS` rows — e.g. the merged regional files). A read slice spanning several input row groups collects into multi-chunk columns; the appended single-chunk `is_dup` column left the columns with mismatched chunk boundaries, which the batched Parquet writer rejects. The pass-2 DataFrame is now chunk-aligned before writing

## [0.12.0] - 2026-07-13

### Changed
- `clean_data.sh` and `dedup_data.sh` now parallelise **per file** by default (one worker per merged file — the finest granularity — each running its whole stage chain in order) instead of per region. Pass `--by-region` for the previous per-region grouping, or `--sequential` for no parallelism. `download_data.sh`/`convert_data.sh` are unchanged (per region)
- `ctddump batch` (convert/header) and `ctddump concat` (convert/header) now treat an empty match — a `--pattern` (or default) matching no files, or an empty/absent input directory — as a no-op: they print an informational message and write no output instead of erroring, so a workflow can reference a not-yet-available dataset
- The `convert_data.sh`/`clean_data.sh`/`dedup_data.sh` Baltic workflow now includes the Global (GL) product (`nrt_bo_gl`), matching the Arctic and Mediterranean. Copernicus does not yet publish Baltic GL data, so those steps currently produce nothing; the scripts skip any file with a missing input (with a note), so the run succeeds and picks up GL automatically once it is available

## [0.11.0] - 2026-07-12

### Added
- The helper scripts run the selected regions **in parallel** by default when more than one is chosen — one background worker per region, with log lines tagged `[region]`. Failures are collected and the script exits non-zero after reporting which region failed. Pass `--sequential` to process regions one at a time

### Changed
- Split `prepare_data.sh` into two scripts with clearer names and prerequisites: **`download_data.sh`** (`login`, `download`; needs only the `copernicusmarine` toolbox) and **`convert_data.sh`** (`process`, `report`; needs only `ctddump`). `download_data.sh` fetches into `-s/--src` (default `input`); `convert_data.sh` writes its summaries to `output/report/convert/` (was `output/report/prepare/`). The pipeline now reads download → convert → clean → dedup

### Removed
- `scripts/prepare_data.sh` (replaced by `download_data.sh` + `convert_data.sh`)

## [0.10.0] - 2026-07-12

### Added
- The helper scripts (`prepare_data.sh`, `clean_data.sh`, `dedup_data.sh`) take command-line options in place of environment variables — `-t`/`--threads`, `-s`/`--src`, `-o`/`--out`. Options may appear anywhere on the command line and accept both `--out DIR` and `--out=DIR` forms
- The helper scripts print the resolved configuration and ask for a `y/N` confirmation before running; `-y`/`--yes` skips the prompt (required in non-interactive shells, which otherwise abort with a hint). Each step is announced with a timestamp so the currently running process is visible
- "Helper scripts" documentation page describing the three-phase pipeline (prepare → clean → dedup), the shared command/region/option interface, the confirmation prompt, and each script's commands and output layout

### Changed
- The helper scripts default to reading source NetCDF from `input/` and writing outputs to `output/` (run from a single working directory), replacing the previous `../source_data/ctddump/netcdf` and `../process_data/ctddump` defaults; the regional workflow docs use the same layout

### Removed
- The helper scripts no longer read the `THREADS`, `SRC`, and `OUT` environment variables; use the `-t`/`-s`/`-o` options instead

## [0.9.0] - 2026-07-11

### Added
- `markdup` subcommand: mark duplicate profiles with an `is_dup` column and write a TSV listing the duplicated profiles. Duplicates are decided by `profile_timestamp` (date only by default), `longitude`, and `latitude` (3-decimal rounding by default), across platforms. The timestamp format (`--time-format`), rounding precision (`--decimals`), and rounding mode (`--round-mode`) are configurable
- `dedup` subcommand: remove duplicate profiles, keeping the one with the most observations (ties broken by first appearance). Re-derives the same key as `markdup` (same options); resets `is_dup` to `false` when the column is present
- `report parquet`: when an `is_dup` column is present, the report adds duplicate counts — `dup_profiles` at the global/platform levels and the `is_dup` flag at the profile level
- `scripts/dedup_data.sh`: helper that de-duplicates the cleaned Parquet outputs of `clean_data.sh` in four steps — `markdup`, `report`, `dedup`, `report` — writing reports to `$OUT/report/dedup/{markdup,dedup}/`
- The regional workflow docs (Arctic, Baltic, Mediterranean) gain a "Data de-duplication" section

### Fixed
- **Critical:** `filter`, `dropqc`, and `dropna` produced corrupt output on any Parquet input with more than one row group (e.g. a `concat` output over ~1M rows): Polars 0.43's parallel Parquet reader mishandles `slice` pushdown across row groups, so every streamed chunk read rows from the *first* row group only. The streaming scans now disable the parallel reader (`ParallelStrategy::None`), which slices correctly; single-row-group inputs were unaffected

## [0.8.1] - 2026-07-11

### Fixed
- `scripts/prepare_data.sh`: remove a stray `W` line (introduced in v0.8.0) that made the script exit immediately with `W: command not found` under `set -e`

## [0.8.0] - 2026-07-11

### Added
- `dropqc` subcommand: drop whole profiles flagged bad by their profile-level QC — a profile is kept only if both `time_qc` and `position_qc` are `"1"` (OK) or missing (an absent QC variable or the NA byte `-128`, both stored as `""`). Missing QC is kept on purpose so files that ship no profile-level QC are not discarded. Works on Parquet files and streams one row group at a time so peak memory stays bounded
- `scripts/clean_data.sh`: helper that cleans the merged Parquet outputs of `prepare_data.sh` in four steps — `dropqc`, `dropna`, `filter` (region bounding box with excluded sub-areas for the Baltic and Mediterranean), and `report` — writing its reports to `$OUT/report/clean/`
- The regional workflow docs (Arctic, Baltic, Mediterranean) gain a "Data cleaning" section covering the `dropqc`/`dropna`/`filter`/`report` pipeline

### Changed
- `scripts/prepare_data.sh` now writes its reports to `$OUT/report/prepare/` (was `$OUT/report/`), leaving `$OUT/report/clean/` for `clean_data.sh`

## [0.7.0] - 2026-07-11

### Added
- `dropna` subcommand: drop whole profiles that are entirely NA (null/NaN) in any of `temp`/`psal`/`pres` — a profile is kept only if each parameter has at least one valid observation. Works on Parquet files in two streaming passes so peak memory stays bounded and the result is independent of chunking

## [0.6.0] - 2026-07-11

### Added
- `filter` subcommand: keep (`--mode include`, default) or drop (`--mode exclude`) whole profiles by a geographic bounding box (`--min-lon`/`--max-lon`/`--min-lat`/`--max-lat`, inclusive edges; NaN positions treated as outside). Works on Parquet files and streams one row group at a time so peak memory stays bounded

### Changed
- `scripts/prepare_data.sh` and the regional workflow pages now generate parquet reports at the `platform` level instead of `global`

## [0.5.0] - 2026-07-10

### Added
- `report` subcommand: summarise a Parquet data file or a YAML header file as a text report (TSV, plain text, or JSON), to a file or stdout
  - `report parquet --level global|platform|profile`: profile/observation counts, per-profile "good" QC counts, missing-value counts, min/max/mean of `temp`/`psal`/`pres`, and the `longitude`/`latitude` bounding box (global/platform levels)
  - `report yaml`: per source file, core-column presence flags and an `extra_params` list of auto-detected non-core measurement parameters — biogeochemical/biological and other (DOXY, FLU2, TUR3, CNDC, …)
- `scripts/prepare_data.sh`: a `report` stage summarising the merged outputs (and `all` now runs download → process → report); the regional workflow pages gain a matching report step

## [0.4.3] - 2026-07-09

### Added
- `RELEASING.md` documenting the release procedure; `README.md` and `CLAUDE.md` now point to it
- Link to the documentation website in `README.md`

## [0.4.2] - 2026-07-09

### Added
- MIT License (`LICENSE`, `license = "MIT"` in `Cargo.toml`, and a README section)

## [0.4.1] - 2026-07-09

### Added
- Project documentation website built with mdBook and auto-deployed to GitHub Pages (<https://aiqc-hub.github.io/ctddump/>): introduction, installation, a page per command, configuration and output-schema reference, and end-to-end regional workflows for the Arctic, Baltic, and Mediterranean seas
- `scripts/prepare_data.sh`: a single runnable data-preparation pipeline (download → convert → merge → header export → merge) built from reusable shell functions, replacing the ad-hoc `data_preparation.md`

## [0.4.0] - 2026-07-09

### Added
- `concat convert --no-pres-sort`: sort without `pres`, keeping each profile's observations in their original source order instead of reordering by pressure
- `concat convert --keep-na-pres`: retain rows whose `pres` is null/NaN
- `concat convert --threads N`: renumber platform ranges in parallel (default: all cores; `--threads 1` selects the sequential, lowest-memory path)
- `CTDDUMP_CHUNK_ROWS` environment variable to tune the streaming chunk / platform-range size

### Changed
- `convert`, `batch convert`, and `concat convert` now stream data in bounded memory — chunked Parquet row groups for conversion and contiguous `platform_code` ranges for `concat` — instead of materializing the full dataset, greatly reducing peak memory on large inputs (output is data-identical; only the on-disk row-group layout changes)
- `concat convert` now drops rows with missing (null/NaN) `pres` by default; pass `--keep-na-pres` to retain them

### Fixed
- Unbounded memory growth in `batch` mode caused by a Polars 0.43.1 parallel-operation leak; batch RSS is now a small constant regardless of file count
- NRT AR/MO files that ship `DEPH` instead of `PRES` now derive `pres` correctly via conversion (previously both `pres` and `deph` could be all-NaN)
- `batch` mode worker stack overflow and thread over-subscription on large files

## [0.3.0] - 2026-05-26

### Added
- `concat convert` command: merge Parquet files from a directory tree into a single file with optional `profile_no` / `observation_no` renumbering (on by default)
- `concat header` command: merge header YAML files into a single YAML file; errors on duplicate keys
- CI workflow: runs tests on push/PR to `main` (GitHub Actions)
- README.md

### Changed
- `concat` is now a subcommand group (`concat convert`, `concat header`) for consistency with `batch`

## [0.2.0] - 2026-05-25

### Added
- `batch convert` command: recursive, multi-threaded NetCDF → Parquet conversion
- `batch header` command: recursive, multi-threaded NetCDF → YAML conversion
- `header` command: single-file NetCDF → YAML metadata extraction
- `--pattern` option on all `batch` subcommands for filename-based file selection (glob, per-subcommand default)
- `--output`, `--threads` options on all `batch` subcommands
- All config fields exposed as individual CLI flags on `convert` and `batch convert`; priority: built-in default < `--config` file < CLI flags
- `profile_longitude` / `profile_latitude` columns in NRT output (from `PRECISE_*` or expanded `DEPLOY_*`)
- Unit tests for pure logic functions (no fixture files required)

### Changed
- QC flag columns changed from `i8` to `String`; non-numeric codes (e.g. `"A"`, `"B"`) are preserved; missing values become `""`
- Source tree reorganised: `src/netcdf/` → `src/convert/`; header modules moved to `src/header/`
- NRT and CORA converters unified into single modules with TOML config files

## [0.1.0] - 2026-05-01

Initial import.

[Unreleased]: https://github.com/AIQC-Hub/ctddump/compare/v0.12.1...HEAD
[0.12.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.3...v0.5.0
[0.4.3]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/AIQC-Hub/ctddump/releases/tag/v0.1.0
