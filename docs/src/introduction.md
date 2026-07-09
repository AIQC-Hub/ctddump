# ctddump

**ctddump** is a small, fast command-line tool for converting oceanographic
**CTD** (Conductivity, Temperature, Depth) data from NetCDF into analysis-ready
formats:

- **Parquet** — the observation data, as a uniform flat table.
- **YAML** — the file metadata (dimensions, variables, global attributes).

It is written in Rust, streams data in bounded memory, and processes whole
directory trees in parallel.

## Data sources

Two families of source files are supported:

| Source | Description |
|--------|-------------|
| **NRT** | Near Real Time — Arctic Sea (`nrt_ar`), Baltic Sea (`nrt_bo`), Mediterranean Sea (`nrt_mo`), Global (`nrt_gl`) |
| **CORA** | Copernicus Ocean Reanalysis — current format (`cora`), legacy format (`cora_legacy`) |

## What you can do

| Command | Purpose |
|---------|---------|
| [`convert`](./commands/convert.md) | Convert a single NetCDF file to Parquet. |
| [`batch`](./commands/batch.md) | Convert a whole directory tree to Parquet or YAML, in parallel. |
| [`header`](./commands/header.md) | Extract a single file's metadata to YAML. |
| [`concat`](./commands/concat.md) | Merge many Parquet (or YAML) files into one. |
| [`report`](./commands/report.md) | Summarise a Parquet or YAML file as a text report. |

## Quick example

```bash
# One file to Parquet
ctddump convert nrt_ar input.nc output.parquet

# A whole Arctic directory, then merge into a single Parquet file
ctddump batch convert nrt_ar --output ./parquet ./netcdf
ctddump concat convert ./parquet arctic.parquet
```

New here? Start with [Installation](./installation.md), skim the
[Commands](./commands/convert.md), then follow an end-to-end
[regional workflow](./examples/arctic.md).
