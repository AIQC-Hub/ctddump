# Helper scripts

The [`scripts/`](https://github.com/AIQC-Hub/ctddump/tree/main/scripts)
directory ships four Bash scripts that automate the full regional pipeline —
the same steps documented one command at a time in the
[Regional workflows](./examples/arctic.md). They run in four phases, each
consuming the previous phase's output:

| Phase | Script | What it does |
|-------|--------|--------------|
| 1. Download | [`download_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/download_data.sh) | Download the source NetCDF from Copernicus Marine into `input/`. |
| 2. Convert | [`convert_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/convert_data.sh) | Convert + merge to Parquet, export + merge headers. |
| 3. Clean | [`clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh) | Drop bad-QC profiles, drop all-NA profiles, restrict to the region. |
| 4. De-duplicate | [`dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh) | Mark duplicate profiles, then remove them. |

Each phase handles the **Arctic**, **Baltic**, and **Mediterranean** regions.
The scripts only orchestrate the `ctddump` binary, so it must be on your `PATH`
— except `download_data.sh`, which instead needs the `copernicusmarine` toolbox
(and a free Copernicus Marine account) and does not use `ctddump` at all.

## Running a script

Run the scripts from a working directory (e.g. `ctddump`). Source NetCDF is read
from `input/` and results are written under `output/`; both are created as
needed. They share one interface:

```
scripts/<script>.sh [options] [command] [region ...]
```

- **command** — the step (or `all`) to run; each script defaults to its most
  common command when omitted (see below).
- **region** — one or more of `arctic`, `baltic`, `mediterranean`. Omitting them
  (or passing `all`) runs every region.
- **options** — may appear anywhere on the line, in either `--out DIR` or
  `--out=DIR` form.

Before doing any work, a script prints the resolved configuration and asks for
confirmation:

```console
$ scripts/clean_data.sh dropqc
Configuration:
  command : dropqc
  regions : arctic baltic mediterranean
  files   : 8
  out     : output
  mode    : parallel (per file)
Proceed? [y/N]
```

Answer `y` to proceed; anything else (including a bare Enter) aborts. Pass
`-y`/`--yes` to skip the prompt — required when running non-interactively (a
pipe or CI job), where the script otherwise aborts with a hint rather than hang.
While running, each step is announced with a timestamp so you can see what is
currently in progress.

### Parallelism

The selected work runs **in parallel** by default, since the units are
independent. Each worker tags its log lines with its region so the interleaved
output stays readable:

```text
[12:00:01] [arctic] dropqc nrt_ar_ar
[12:00:01] [arctic] dropqc nrt_ar_gl
[12:00:01] [baltic] dropqc nrt_bo_bo
```

The granularity differs by script:

- **`clean_data.sh` and `dedup_data.sh`** default to **per file** — one worker
  per merged file (a *stem* within a region), the finest granularity (8 files
  across the three regions). Each file runs its whole stage chain (e.g.
  `dropqc → dropna → filter → report`) in order. Pass `--by-region` for one
  worker per region instead (coarser), or `--sequential` for no parallelism.
- **`download_data.sh` and `convert_data.sh`** parallelise **per region**;
  `--sequential` disables it.

If any unit fails, the others still finish and the script exits non-zero after
reporting which unit failed. A run with only one unit is always sequential.
Note that `convert`/`clean`/`dedup` each also use multiple threads *within* a
unit, so parallel units multiply the load accordingly — throttle with
`--by-region` / `--sequential`, or `-t/--threads` for `convert`. For
`download_data.sh`, `--sequential` also helps if concurrent Copernicus transfers
hit rate limits.

### Options

| Option | Scripts | Default | Meaning |
|--------|---------|---------|---------|
| `-t, --threads N` | convert | `10` | Worker threads for `ctddump`. |
| `-s, --src DIR` | download, convert | `input` | Root of the source NetCDF tree (downloaded into / read from). |
| `-o, --out DIR` | convert, clean, dedup | `output` | Root for the generated / consumed outputs. |
| `--by-region` | clean, dedup | — | Parallelise per region instead of per file. |
| `--sequential` | all | — | Process one unit at a time (no parallelism). |
| `-y, --yes` | all | — | Skip the confirmation prompt. |
| `-h, --help` | all | — | Show the script's help. |

### Missing datasets

Every stage tolerates missing inputs, so an unavailable dataset does not fail the
run. A `ctddump batch` or `concat` step that matches no files reports this and
writes nothing; `clean_data.sh` / `dedup_data.sh` skip any file whose input is
missing (with a note). The concrete case today is the **Global (GL) product for
the Baltic Sea**, which Copernicus does not yet publish: the `nrt_bo_gl` outputs
are simply skipped, and appear automatically once the data becomes available.

## `download_data.sh`

Downloads the source NetCDF from Copernicus Marine. Commands: `login`,
`download` *(default)*. Needs the `copernicusmarine` toolbox (not `ctddump`).

```bash
scripts/download_data.sh login             # one-time Copernicus login
scripts/download_data.sh                   # download every region into input/
scripts/download_data.sh download arctic   # just the Arctic
```

Each product's directory is downloaded under `input/` (override with
`-s/--src`), ready for `convert_data.sh`.

## `convert_data.sh`

Converts the downloaded NetCDF to Parquet and metadata. Commands: `process`
*(default)*, `report`, `all`.

```bash
scripts/convert_data.sh all                # process + report, all regions
scripts/convert_data.sh process arctic     # just convert + merge the Arctic
```

Reads the source NetCDF from `input/` (`-s/--src`); writes merged Parquet to
`output/parquet/`, merged headers to `output/header/`, and summaries to
`output/report/convert/`.

## `clean_data.sh`

Cleans the merged Parquet from phase 1. Commands: `dropqc`, `dropna`, `filter`,
`report`, `all` *(default)*. The stages chain
`output/parquet → clean/dropqc → clean/dropna → clean/filter`, with summaries in
`output/report/clean/`. The `filter` step applies each region's bounding box(es).

```bash
scripts/clean_data.sh                       # dropqc → dropna → filter → report, all regions
scripts/clean_data.sh -y baltic             # skip the prompt, Baltic only
```

## `dedup_data.sh`

Removes duplicate profiles from the cleaned data. Commands: `markdup`, `report`,
`dedup`, `all` *(default)*. It reads `output/clean/filter`, writes marked data to
`output/dedup/markdup/` (plus a duplicates TSV) and de-duplicated data to
`output/dedup/dedup/`, with summaries under `output/report/dedup/`.

```bash
scripts/dedup_data.sh                        # markdup → report → dedup → report, all regions
```

## Full pipeline

Run the four phases in order (skipping prompts) for every region. Log in once
first if you have not already (`scripts/download_data.sh login`):

```bash
scripts/download_data.sh -y            # download every region into input/
scripts/convert_data.sh  -y all
scripts/clean_data.sh    -y all
scripts/dedup_data.sh    -y all
```

For the equivalent commands spelled out step by step, see the
[Regional workflows](./examples/arctic.md).
