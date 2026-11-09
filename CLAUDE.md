# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

`ctddump` is a Rust CLI that converts oceanographic CTD (Conductivity, Temperature, Depth) data from NetCDF to Parquet (data) or YAML (metadata). Two source repositories:
- **NRT** (Near Real Time): regional data, Arctic (AR), Baltic (BO), Mediterranean (MO), Global (GL).
- **CORA** (Copernicus Ocean Reanalysis): historical re-processed profiles (`cora` current format, `cora_legacy` older files).

Commands: `convert`, `batch`, `header`, `checkpres`, `concat`, `report`, `filter`, `dropna`, `dropqc`, `markdup`, `dedup`, `compare`, `sqlite`. The full interface is defined with `clap` in `src/cli.rs` and self-documents via `--help` at every level.

## House rules (hard constraints)

- **No em dashes** in any human-facing document: `README.md`, the mdBook under `docs/`, `CHANGELOG.md`, `RELEASING.md`, the generated reports/summary pages, and the summary web site. This covers both committed prose and text the tools emit at runtime (report/summary renderers, script log lines, help text). Use a colon, comma, parentheses, semicolon, or reword instead.
- **Git:** commit day-to-day work directly to `develop`; `main` is stable releases. Commit and push **only when the user asks**. `git-flow` (AVH) is installed for multi-session features/hotfixes, use it only when it adds value. Releases are cut by merging `develop` into `main` and tagging `vX.Y.Z` (see `RELEASING.md`), not via `git flow release`.
- Commit messages end with `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

## Build & test

```bash
cargo build                        # build
cargo build --features trace       # build with debug-logging feature
cargo test                         # run all tests
cargo test test_convert_nrt_ar_1   # run one test by name
cargo run -- <cmd> [subcmd] --help # discover any command's interface
```

**System deps:** the `netcdf` crate links HDF5 via `hdf5-metno-sys`. Install dev headers first: `sudo apt-get install libhdf5-dev libnetcdf-dev` (Ubuntu/Debian) or `brew install hdf5` (macOS). HDF5 1.10.10 (CI runner) emits harmless `HDF5-DIAG` messages on files with the 1.12 `_QuantizeBitGroomNumberOfSignificantDigits` attribute; data reads correctly and tests are unaffected.

**Test data:** fixtures under `tests/` are not committed. Fetch with `scripts/fetch_test_data.sh` (needs authenticated `gh` CLI and `unzip`; override with `TEST_DATA_VERSION` / `TEST_DATA_REPO`).

## Where to read more

- **[docs/dev/architecture.md](docs/dev/architecture.md)** — source-tree map, per-command/module internals, converter streaming design, the Polars 0.43.1 workarounds (leak + slice-pushdown), threading/stack notes, output DataFrame schema, `trace` feature.
- **[docs/dev/cli-behavior.md](docs/dev/cli-behavior.md)** — non-obvious CLI behavior: config precedence, NRT region defaults, DEPH auto-detect, CORA legacy defaults, `--pattern`, no-op-on-empty, `batch` output rules, `concat` renumber/pres flags.
- **mdBook (`docs/src/`)** — the user-facing docs: per-command pages, `configuration.md` (config precedence + environment variables), `output-schema.md`, `technical-notes.md` (memory/threading/Polars notes for a general audience), and regional workflow examples.
