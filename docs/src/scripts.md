# Helper scripts

The [`scripts/`](https://github.com/AIQC-Hub/ctddump/tree/main/scripts)
directory ships three Bash scripts that automate the full regional pipeline —
the same steps documented one command at a time in the
[Regional workflows](./examples/arctic.md). They run in three phases, each
consuming the previous phase's output:

| Phase | Script | What it does |
|-------|--------|--------------|
| 1. Prepare | [`prepare_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/prepare_data.sh) | Download the source NetCDF, convert + merge to Parquet, export + merge headers. |
| 2. Clean | [`clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh) | Drop bad-QC profiles, drop all-NA profiles, restrict to the region. |
| 3. De-duplicate | [`dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh) | Mark duplicate profiles, then remove them. |

Each phase handles the **Arctic**, **Baltic**, and **Mediterranean** regions.
The scripts only orchestrate the `ctddump` binary, so it must be on your `PATH`
(`prepare_data.sh` also needs the `copernicusmarine` toolbox for downloads).

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
$ scripts/clean_data.sh dropqc arctic
Configuration:
  command : dropqc
  regions : arctic
  out     : output
Proceed? [y/N]
```

Answer `y` to proceed; anything else (including a bare Enter) aborts. Pass
`-y`/`--yes` to skip the prompt — required when running non-interactively (a
pipe or CI job), where the script otherwise aborts with a hint rather than hang.
While running, each step is announced with a timestamp so you can see what is
currently in progress.

### Options

| Option | Scripts | Default | Meaning |
|--------|---------|---------|---------|
| `-t, --threads N` | prepare | `10` | Worker threads for `ctddump`. |
| `-s, --src DIR` | prepare | `input` | Root of the downloaded NetCDF tree. |
| `-o, --out DIR` | all | `output` | Root for the generated / consumed outputs. |
| `-y, --yes` | all | — | Skip the confirmation prompt. |
| `-h, --help` | all | — | Show the script's help. |

## `prepare_data.sh`

Downloads and converts the source data. Commands: `login`, `download`,
`process` *(default)*, `report`, `all`.

```bash
scripts/prepare_data.sh login              # one-time Copernicus login
scripts/prepare_data.sh all                # download, process, report — all regions
scripts/prepare_data.sh process arctic     # just convert + merge the Arctic
```

Writes merged Parquet to `output/parquet/`, merged headers to `output/header/`,
and summaries to `output/report/prepare/`.

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

Run the three phases in order (skipping prompts) for every region:

```bash
scripts/prepare_data.sh -y all
scripts/clean_data.sh   -y all
scripts/dedup_data.sh   -y all
```

For the equivalent commands spelled out step by step, see the
[Regional workflows](./examples/arctic.md).
