# Changelog

## [Unreleased]

### Added
- `report` subcommand: summarise a Parquet data file or a YAML header file as a text report (TSV, plain text, or JSON), to a file or stdout
  - `report parquet --level global|platform|profile`: profile/observation counts, per-profile "good" QC counts, missing-value counts, and min/max/mean of `temp`/`psal`/`pres`
  - `report yaml`: per source file, core-column presence flags and an `extra_params` list of auto-detected non-core measurement parameters — biogeochemical/biological and other (DOXY, FLU2, TUR3, CNDC, …)

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

[Unreleased]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.3...HEAD
[0.4.3]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/AIQC-Hub/ctddump/releases/tag/v0.1.0
