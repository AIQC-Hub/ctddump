# Changelog

## [Unreleased]

## [0.27.0] - 2026-07-19

### Added
- `compare` subcommand: compares two Parquet files and reports how far each covers the other's platforms and profiles. Profiles are matched on the same key as `markdup` (time reduced to a date, longitude/latitude rounded to 3 decimals) except that `platform_code` is part of the key by default, since two compared files are usually extracts of the same platforms; `--no-platform-key` matches on time and position alone. Coverage is reported in both directions, because it is not symmetric: a file contained in a larger one is fully covered while covering only part of it. The second file is the reference in the first output row. For the profiles found in both files the report also says whether they carry the same number of observations, which separates "the same profiles with different cleaning applied" from "different profiles". The key column names, the time format, the rounding decimals, and the rounding mode are all configurable, and the time column may be either a datetime (`profile_timestamp`) or a float of days since 1950 (`profile_time`). Output is TSV (default), aligned text, or JSON. Each file is streamed and reduced to one record per profile, so memory follows the profile count rather than the file size

## [0.26.1] - 2026-07-19

### Changed
- The installation guide leads with the prebuilt binaries instead of `cargo install`, since they are the right route for most users and need neither a Rust toolchain nor HDF5 headers. It opens with a table of the three routes, then covers downloading and verifying an archive against `SHA256SUMS`, putting the binary on `PATH`, and what else the archive contains (the helper scripts and which external tools they still need). The system dependencies are now scoped to the two routes that compile ctddump

### Fixed
- The prebuilt-binary runner table in `RELEASING.md` still named `macos-13` and `macos-14`, which the workflow stopped using in 0.26.0. It now matches `publish.yml` (`macos-15-intel` and `macos-15`)

## [0.26.0] - 2026-07-18

### Added
- Prebuilt binaries attached to each GitHub Release, so ctddump can be used without a Rust toolchain. A `vX.Y.Z` tag now builds an archive for `x86_64`/`aarch64` Linux and `x86_64`/`aarch64` macOS and attaches them with `SHA256SUMS`. Each archive holds the stripped binary, the helper scripts, `README.md`, `LICENSE`, and `CHANGELOG.md` (about 13 MB compressed). The build enables `static-netcdf`, so the binary needs no system libraries: on Linux it links only glibc and the loader. Note the bundled scripts are not self-contained: the four pipeline scripts need only `ctddump` on `PATH`, but `download_data.sh` needs `copernicusmarine`, `summary_site.sh` needs `mdbook`, and `fetch_test_data.sh` needs `gh` and `unzip`
- `static-netcdf` feature, which builds HDF5 and netCDF from source instead of linking the system libraries, and `[package.metadata.docs.rs]` telling docs.rs to use it. docs.rs has no libnetcdf and cannot install one, so builds there failed in the `netcdf-sys` build script and the crate had no rendered API docs. The feature is off by default: ordinary builds and `cargo install ctddump` should keep installing `libhdf5-dev` / `libnetcdf-dev`, which is far quicker. Since docs.rs metadata is baked into each published version, this takes effect from this release onwards and cannot fix 0.24.2 or 0.25.0

## [0.25.0] - 2026-07-18

### Added
- Automated publishing to crates.io. Pushing a `vX.Y.Z` tag, already the last step of the release procedure, runs the test suite and uploads the crate. Authentication uses crates.io Trusted Publishing (OIDC), so no long-lived API token is stored in the repository. Because publishing cannot be undone, the job is gated: the tests run first rather than assuming CI covered the tagged commit, the job fails if the tag disagrees with the `Cargo.toml` version, and `cargo publish --dry-run --locked` builds the real tarball before the upload. Manual runs from the Actions tab default to a dry run
- Installation from crates.io is now documented: `cargo install ctddump` leads both the README and the installation guide, with the system dependencies still called out first and an escape hatch (`NETCDF_DIR` / `HDF5_DIR`) for the "A system version of libnetcdf could not be found" failure. The README also gains a crates.io version badge and link

### Changed
- `RELEASING.md` documents what a tag push now triggers, the one-time Trusted Publishing setup that must stay in sync between crates.io and the GitHub environment, and the window between pushing `main` and pushing the tag where the publish workflow can be rehearsed as a dry run

## [0.24.2] - 2026-07-18

### Fixed
- CI no longer runs the runner out of disk while saving its cache. The cache step tarred all of `target/`, which holds 20 integration-test binaries that each statically link their own copy of Polars, and zstd failed writing the tarball. The job now uses `Swatinem/rust-cache`, which caches only dependency artifacts (dropping `incremental/` and this workspace's own crates before saving), reclaims more runner disk up front, and deletes the test fixtures and `incremental/` after the tests so the cache save has room. `actions/checkout` moves to v7, so every pinned action runs on Node 24

## [0.24.1] - 2026-07-18

### Added
- Package metadata for publication to crates.io: `description` (required by the registry), plus `repository`, `homepage`, `documentation`, `readme`, `keywords`, and `categories` for the crate page. An `exclude` list keeps development-only material (`CLAUDE.md`, `.claudeignore`, `.github/`, `docs/`, `scripts/`, `RELEASING.md`) out of the published tarball, taking it from 84 files to 51. The docs and helper scripts remain in the repository, and the hosted documentation is unaffected

## [0.24.0] - 2026-07-18

### Added
- The "Filter by region" section of `report summary` gains a **Bounding box** table: the minimum and maximum longitude and latitude of the profiles that survived the filter, in decimal degrees. The extremes are aggregated from the per-platform extent columns of the stage TSV, so they span every platform in the file. When the Conversion report is present a second `Original` column gives the same extremes before any cleaning ran, showing how far the filter tightened the box. Profiles with a missing position are ignored, and the table is omitted when no profile has a valid position (so stage TSVs without the extent columns are unaffected)

## [0.23.0] - 2026-07-18

### Added
- `convert_data.sh`, `clean_data.sh`, and `dedup_data.sh` gain `--time-log FILE`: the `--time` measurements are written to FILE (the option implies `--time`) instead of the screen, so normal progress stays visible. The file is created fresh each run, and in parallel mode each worker appends its own `timed …` lines. A bad path fails fast up front

## [0.22.1] - 2026-07-17

### Changed
- Removed em dashes from all human-facing documentation for a consistent house style: `README.md` (whose documentation links are now a bulleted list) and every page under `docs/src/`. The no-em-dash convention is recorded in `CLAUDE.md`

## [0.22.0] - 2026-07-17

### Added
- `convert_data.sh`, `clean_data.sh`, and `dedup_data.sh` gain a `--time` option (off by default): each `ctddump` step is wrapped in GNU time and its wall-clock seconds and peak resident memory are logged as a `timed <step>: …` line. Requires GNU time (the `time` package, not the shell builtin); the scripts resolve `/usr/bin/time` or `gtime` and verify it up front, and honour `CTDDUMP_TIME_BIN`. Peak RSS is per `ctddump` process; for comparable per-step wall times pair it with `--sequential`

## [0.21.1] - 2026-07-17

### Added
- Link the published live example report site (<https://aiqc-hub.github.io/ctddump-report-example/>) from the README and the `summary_site.sh` section of the docs, as a static, point-in-time sample of that pipeline phase's output

## [0.21.0] - 2026-07-17

### Added
- `summary_site.sh` now makes the built site directory a self-contained, publishable repository: alongside the `.nojekyll` that mdBook already writes, it copies in a `LICENSE` (the project's own by default; `-l/--license FILE` to override, `--license ""` to skip) and writes a short `README.md` describing the site. This lets the output directory be pushed straight to a publishing repository (e.g. `ctddump-report-example`)

## [0.20.0] - 2026-07-17

### Changed
- `summary_site.sh`'s built-in book title default is now `ctddump: CTD data summary reports` (was `CTD data summary reports`), so the generated site identifies itself as ctddump output. Still overridable with `-t/--title` and ignored when a custom `--config` is supplied

## [0.19.0] - 2026-07-17

### Added
- The three labelled Mark-duplicates tables in `report summary` (Duplicates within a platform, Duplicates across platforms, Duplicate group sizes) each carry a short explanation of what they show, alongside the existing per-section prose
- `summary_site.sh` helper script: renders the Markdown summary pages from `summary_data.sh` into a static local web site with [mdBook](https://rust-lang.github.io/mdBook/). Reads pages from `-s/--src` (default `summary`) and writes the built site to `-d/--dest` (default `site`), grouping chapters into one part per region. Each chapter's name is taken from the page's own top-level heading, so titles stay defined in one place. A `book.toml` is written for you, or pass `-c/--config FILE` to use your own (`-t/--title` sets the title of the built-in template). A region with no pages is skipped; no pages at all is an error rather than an empty site. Requires mdbook on PATH

### Fixed
- `report yaml` reported an inconsistent pair: `has_time` tested for the `TIME` data variable while `has_position` tested for the `POSITION_QC` flag, so the `report summary` File summary listed "with TIME" beside "with POSITION_QC". The two columns are now the profile-level QC pair they were meant to be, renamed `has_time_qc` / `has_position_qc` (testing `TIME_QC` / `POSITION_QC`), matching the `time_qc` / `position_qc` output columns and what `dropqc` filters on. **Breaking** for anything consuming the `report yaml` TSV by column name

### Changed
- Every helper script's configuration block now ends with `Run with -h/--help to see all options.`, so the full option list is discoverable from the confirmation prompt that each script prints by default
- Generated summary pages no longer use em dashes. The section prose is reworded, the pipeline page titles now separate region and product with a colon (`Arctic Ocean: CORA reanalysis`), and an empty "Extra parameters" cell reads `none` instead of a dash. Guarded by a test over both the Markdown and HTML renderers

## [0.18.0] - 2026-07-16

### Added
- `report summary` gains `--title TEXT`, replacing the default `Summary: <stem>` page heading with a human-readable one, and `--note TEXT` (repeatable), rendering caller-supplied notes under the title. Both are plain text and escaped in HTML output, and are the place for region- or product-specific remarks
- Every `report summary` section now carries a short explanation of what the stage did. The prose is generic across regions and datasets; anything specific belongs in a `--note`
- The `report summary` Mark-duplicates section gains a **Duplicate group sizes** table — how many groups hold 2, 3, … profiles, with one row per distinct size up to 10 and an `11+` row for the tail
- `summary_data.sh` now gives each page a human-readable title and product-specific notes, defined per stem by `title_for` / `notes_for` near the top of the script

### Changed
- The `report summary` Deduplication stages are now also compared against the **cleaned** data they actually ran on, not only the original: their tables gain `Cleaned`, `% of cleaned` and `Deleted (cleaned)` columns, taken from the last cleaning stage present (Filter, else Drop all-NA, else Drop bad QC). When no cleaning stage ran, the columns are omitted
- The Mark-duplicates "Duplicate profiles" row now reports its share under the percentage columns and leaves the `Deleted` cells blank, since that stage removes nothing (it previously printed the share in the `Deleted` column)

## [0.17.0] - 2026-07-15

### Added
- `report summary` subcommand: assembles a single Markdown (default) or HTML (`--format html`) page for one file stem from the per-stage TSV reports the pipeline already produced. Given a stem it auto-locates the section source files under `--report-dir` (default `report`) and `--out-dir` (default `output`) — File summary (header YAML report), Conversion, Cleaning (Drop bad QC / Drop all-NA / Filter by region), and Deduplication (Mark duplicates / Remove duplicates) — and skips any section whose file is absent. Stage tables show platforms/profiles/observations with `% of original` and `Deleted` columns relative to the Conversion baseline; the Mark-duplicates section additionally splits duplicates into within-platform (a `dup_group` confined to one platform) and across-platform (spanning two or more) from the markdup `.dups.tsv`. Output goes to `-o/--output` or stdout
- `summary_data.sh` helper script: builds one `report summary` page per *(region, product)* unit for the Arctic, Baltic, and Mediterranean, reading reports from `-r/--report` (default `report`) and the markdup `.dups.tsv` from `-o/--out` (default `output`), writing pages to `-d/--dest` (default `summary`) in `-f/--format` `md` or `html`. A stem with no reports is skipped

## [0.16.0] - 2026-07-14

### Changed
- `convert_data.sh` now parallelises **per unit** — one worker per *(region, product)*, where each region's three products are its regional NRT, the Global (GL), and CORA (9 units across the three regions) — instead of per region. Each unit runs its own convert → merge → header → merge-header chain in order. Pass `--by-region` for the previous per-region grouping (its three products in order), or `--sequential` for no parallelism
- `convert_data.sh`'s `-t/--threads` default drops from `10` to `2`, since up to 9 units may now run at once (each `ctddump` call within a unit still uses this many worker threads). The refactor from hard-coded per-region functions to a generic per-unit pipeline is verified command-identical to the previous script for both `process` and `report`

## [0.15.0] - 2026-07-14

### Changed
- Reworked the helper-script directory layout. The source NetCDF tree is now `source/` (was `input/`; the `-s/--src` default in `download_data.sh`/`convert_data.sh`) and the first-stage Parquet dir is `output/convert/` (was `output/parquet/`), so each `output/` sub-directory matches a command name. Summary reports move out of `output/` into a top-level `report/` sibling, selectable with a new `-r/--report DIR` option (default `report`) on `convert_data.sh`/`clean_data.sh`/`dedup_data.sh`
- The `report/` tree mirrors `output/` minus the regional sub-directories: `convert_data.sh` now writes Parquet-data summaries to `report/convert/` and header (YAML) summaries to `report/header/` (previously both landed together in `output/report/convert/`), and `clean_data.sh` now summarises **every** stage into `report/clean/{dropqc,dropna,filter}` (previously only the final `filter` output), interleaved into its `all` chain like `dedup_data.sh`'s per-stage reports (`report/dedup/{markdup,dedup}`)

## [0.14.0] - 2026-07-14

### Added
- `convert_data.sh`, `clean_data.sh`, and `dedup_data.sh` accept `--chunk-rows N`, which exports `CTDDUMP_CHUNK_ROWS` for every `ctddump` process the script launches — a per-run knob to trade memory for Parquet row-group count without editing files. Omitting it keeps `ctddump`'s built-in default, and a value already in the environment is respected. The resolved value is shown in each script's configuration block and `--help`
- "Environment variables" section in the Configuration docs documenting the three tuning variables `ctddump` respects (`CTDDUMP_CHUNK_ROWS`, `POLARS_MAX_THREADS`, `RUST_MIN_STACK`) — what each does and its default — with pointers from the Technical notes and Helper scripts pages

## [0.13.0] - 2026-07-14

### Added
- "Technical notes" documentation page: a plain-English, symptom/why/fix summary of the non-obvious problems encountered while building `ctddump` — streaming and memory use, the Polars parallel-op memory leak and multi-row-group slice-pushdown bug, the `markdup` chunk-alignment crash, batch threading (worker stack size, pinning Polars' internal threads, largest-first scheduling), `concat`'s per-platform streaming, empty-dataset tolerance, and the harmless HDF5 diagnostics

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

[Unreleased]: https://github.com/AIQC-Hub/ctddump/compare/v0.27.0...HEAD
[0.27.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.26.1...v0.27.0
[0.26.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.26.0...v0.26.1
[0.26.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.24.2...v0.25.0
[0.24.2]: https://github.com/AIQC-Hub/ctddump/compare/v0.24.1...v0.24.2
[0.24.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.24.0...v0.24.1
[0.24.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.22.1...v0.23.0
[0.22.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.22.0...v0.22.1
[0.22.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.21.1...v0.22.0
[0.21.1]: https://github.com/AIQC-Hub/ctddump/compare/v0.21.0...v0.21.1
[0.21.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/AIQC-Hub/ctddump/compare/v0.12.2...v0.13.0
[0.12.2]: https://github.com/AIQC-Hub/ctddump/compare/v0.12.1...v0.12.2
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
