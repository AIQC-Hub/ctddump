# Changelog

## [Unreleased]

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

[Unreleased]: https://github.com/AIQC-Hub/ctddump/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/AIQC-Hub/ctddump/releases/tag/v0.1.0
