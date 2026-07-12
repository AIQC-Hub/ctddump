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
  out     : output
  mode    : parallel (per region)
Proceed? [y/N]
```

Answer `y` to proceed; anything else (including a bare Enter) aborts. Pass
`-y`/`--yes` to skip the prompt — required when running non-interactively (a
pipe or CI job), where the script otherwise aborts with a hint rather than hang.
While running, each step is announced with a timestamp so you can see what is
currently in progress.

### Region parallelism

When more than one region is selected (the default), the regions run **in
parallel** — one background worker each — since they are independent units of
work. Each worker tags its log lines with its region so the interleaved output
stays readable:

```text
[12:00:01] [arctic] dropqc arctic/nrt_ar_ar
[12:00:01] [baltic] dropqc baltic/nrt_bo_bo
[12:00:01] [mediterranean] dropqc mediterranean/nrt_mo_mo
```

If any region fails, the others still finish and the script exits non-zero after
reporting which region failed. Pass `--sequential` to process regions one at a
time instead (lower peak resource use; a single-region run is always
sequential). Note that `convert`/`clean`/`dedup` each already use multiple
threads *within* a region, so parallel regions multiply the load accordingly —
tune with `-t/--threads` (convert) or `--sequential`. For `download_data.sh`,
`--sequential` is also useful if concurrent Copernicus transfers hit rate limits.

### Options

| Option | Scripts | Default | Meaning |
|--------|---------|---------|---------|
| `-t, --threads N` | convert | `10` | Worker threads for `ctddump`. |
| `-s, --src DIR` | download, convert | `input` | Root of the source NetCDF tree (downloaded into / read from). |
| `-o, --out DIR` | convert, clean, dedup | `output` | Root for the generated / consumed outputs. |
| `--sequential` | all | — | Process regions one at a time instead of in parallel. |
| `-y, --yes` | all | — | Skip the confirmation prompt. |
| `-h, --help` | all | — | Show the script's help. |

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
