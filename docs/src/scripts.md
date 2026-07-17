# Helper scripts

The [`scripts/`](https://github.com/AIQC-Hub/ctddump/tree/main/scripts)
directory ships six Bash scripts that automate the full regional pipeline —
the same steps documented one command at a time in the
[Regional workflows](./examples/arctic.md). They run in six phases, each
consuming the previous phase's output:

| Phase | Script | What it does |
|-------|--------|--------------|
| 1. Download | [`download_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/download_data.sh) | Download the source NetCDF from Copernicus Marine into `source/`. |
| 2. Convert | [`convert_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/convert_data.sh) | Convert + merge to Parquet, export + merge headers. |
| 3. Clean | [`clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh) | Drop bad-QC profiles, drop all-NA profiles, restrict to the region. |
| 4. De-duplicate | [`dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh) | Mark duplicate profiles, then remove them. |
| 5. Summarise | [`summary_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/summary_data.sh) | Build a per-unit Markdown/HTML summary page from the reports. |
| 6. Publish | [`summary_site.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/summary_site.sh) | Render the Markdown summary pages into a static mdBook site. |

Each phase handles the **Arctic**, **Baltic**, and **Mediterranean** regions.
The scripts only orchestrate the `ctddump` binary, so it must be on your `PATH`
— except `download_data.sh`, which instead needs the `copernicusmarine` toolbox
(and a free Copernicus Marine account) and does not use `ctddump` at all, and
`summary_site.sh`, which needs [mdBook](https://rust-lang.github.io/mdBook/)
(`cargo install mdbook`).

## Running a script

Run the scripts from a working directory (e.g. `ctddump`). Source NetCDF is read
from `source/`, data products are written under `output/`, and summary reports
under `report/`; all are created as needed. They share one interface:

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
  report  : report
  chunk   : default
  mode    : parallel (per file)
Run with -h/--help to see all options.
Proceed? [y/N]
```

Answer `y` to proceed; anything else (including a bare Enter) aborts. Pass
`-y`/`--yes` to skip the prompt — required when running non-interactively (a
pipe or CI job), where the script otherwise aborts with a hint rather than hang.
Every script closes its configuration block with the `-h`/`--help` reminder shown
above, so the full option list is always one step away.
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
- **`convert_data.sh`** defaults to **per unit** — one worker per
  *(region, product)*, where each region's three products are its regional NRT,
  the Global (GL), and CORA (9 units across the three regions). Each unit runs
  its own convert → merge → header → merge-header chain in order. Pass
  `--by-region` for one worker per region instead (its three products in order),
  or `--sequential` for no parallelism.
- **`download_data.sh`** parallelises **per region**; `--sequential` disables it.

If any unit fails, the others still finish and the script exits non-zero after
reporting which unit failed. A run with only one unit is always sequential.
Note that `convert`/`clean`/`dedup` each also use multiple threads *within* a
unit, so parallel units multiply the load accordingly — `convert_data.sh`'s
`-t/--threads` therefore defaults to just `2`, since up to 9 units may run at
once. Throttle further with `--by-region` / `--sequential`. For
`download_data.sh`, `--sequential` also helps if concurrent Copernicus transfers
hit rate limits.

If memory is tight (parallel units each hold a chunk in memory at once), lower
the streaming chunk size with `--chunk-rows N` on `convert`/`clean`/`dedup` — it
trades a smaller memory footprint for more Parquet row groups without changing
the output. See [Configuration](./configuration.md#environment-variables) for the
full list of tuning variables.

### Options

| Option | Scripts | Default | Meaning |
|--------|---------|---------|---------|
| `-t, --threads N` | convert | `2` | Worker threads for each `ctddump` call (kept low because up to 9 units run in parallel by default). |
| `-s, --src DIR` | download, convert | `source` | Root of the source NetCDF tree (downloaded into / read from). |
| `-s, --src DIR` | site | `summary` | Directory holding the `<stem>.md` summary pages to render. |
| `-o, --out DIR` | convert, clean, dedup, summary | `output` | Root for the generated / consumed data outputs (summary reads the markdup `.dups.tsv` from here). |
| `-r, --report DIR` | convert, clean, dedup, summary | `report` | Root for the summary TSV reports (a sibling of `output/`). |
| `-d, --dest DIR` | summary | `summary` | Directory the generated summary pages are written to. |
| `-d, --dest DIR` | site | `site` | Directory the built static site is written to. |
| `-c, --config FILE` | site | built-in | Custom `book.toml` to use instead of the built-in template. |
| `-t, --title TEXT` | site | `ctddump: CTD data summary reports` | Book title (built-in template only; ignored with `--config`). |
| `-l, --license FILE` | site | the `LICENSE` beside the script | LICENSE copied into the built site. Pass `--license ""` to skip it. |
| `-f, --format FMT` | summary | `md` | Summary page format: `md` or `html`. |
| `--chunk-rows N` | convert, clean, dedup | `ctddump` default | Streaming chunk size in rows — lower uses less memory but writes more row groups. Exported as [`CTDDUMP_CHUNK_ROWS`](./configuration.md#environment-variables) for every `ctddump` process the script launches. |
| `--by-region` | convert, clean, dedup | — | Parallelise per region instead of per unit/file. |
| `--sequential` | convert, clean, dedup | — | Process one unit at a time (no parallelism). |
| `--time` | convert, clean, dedup | off | Measure each `ctddump` step with GNU time and log its wall clock and peak memory (see [Timing steps](#timing-steps)). |
| `-y, --yes` | all | — | Skip the confirmation prompt. |
| `-h, --help` | all | — | Show the script's help. |

### Timing steps

`convert_data.sh`, `clean_data.sh`, and `dedup_data.sh` accept `--time` (off by
default). When on, every `ctddump` step is wrapped in [GNU
time](https://www.gnu.org/software/time/) and its wall-clock seconds and peak
resident memory are logged as a `timed <step>: …s, … MiB peak RSS, …% CPU` line
after the step completes:

```text
[12:00:03] dropqc nrt_ar_ar
[12:00:15] timed dropqc nrt_ar_ar: 11.72s, 512 MiB peak RSS, 148% CPU
```

Notes:

- It requires **GNU time** (the `time` package: `sudo apt-get install time`), not
  the shell `time` builtin, which cannot report memory. The scripts resolve
  `/usr/bin/time` (or `gtime`), and check it up front so a missing tool fails fast
  rather than mid-run. Override the binary with the `CTDDUMP_TIME_BIN` environment
  variable.
- Peak RSS is measured **per `ctddump` process**. A multi-step stage such as the
  multi-box `filter` logs one line per underlying process (`… (include)`, `…
  (exclude 1)`, …).
- Under the default parallelism, wall-clock times of concurrent steps overlap and
  CPU% can exceed 100% (each `ctddump` is itself multi-threaded). For clean,
  comparable per-step wall times, pair `--time` with `--sequential`; peak RSS is
  per process and meaningful either way.

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
scripts/download_data.sh                   # download every region into source/
scripts/download_data.sh download arctic   # just the Arctic
```

Each product's directory is downloaded under `source/` (override with
`-s/--src`), ready for `convert_data.sh`.

## `convert_data.sh`

Converts the downloaded NetCDF to Parquet and metadata. Commands: `process`
*(default)*, `report`, `all`.

```bash
scripts/convert_data.sh all                # process + report, all regions
scripts/convert_data.sh process arctic     # just convert + merge the Arctic
```

Reads the source NetCDF from `source/` (`-s/--src`); writes merged Parquet to
`output/convert/`, merged headers to `output/header/`, and summaries to
`report/convert/` (Parquet data) and `report/header/` (header YAML).

## `clean_data.sh`

Cleans the merged Parquet from phase 1. Commands: `dropqc`, `dropna`, `filter`,
`report`, `all` *(default)*. The stages chain
`output/convert → clean/dropqc → clean/dropna → clean/filter`, with a summary of
each stage in `report/clean/{dropqc,dropna,filter}/`. The `filter` step applies
each region's bounding box(es).

```bash
scripts/clean_data.sh                       # dropqc → dropna → filter → report, all regions
scripts/clean_data.sh -y baltic             # skip the prompt, Baltic only
```

## `dedup_data.sh`

Removes duplicate profiles from the cleaned data. Commands: `markdup`, `report`,
`dedup`, `all` *(default)*. It reads `output/clean/filter`, writes marked data to
`output/dedup/markdup/` (plus a duplicates TSV) and de-duplicated data to
`output/dedup/dedup/`, with summaries under `report/dedup/{markdup,dedup}/`.

```bash
scripts/dedup_data.sh                        # markdup → report → dedup → report, all regions
```

## `summary_data.sh`

Builds one [`report summary`](./commands/report.md#report-summary) page per
*(region, product)* unit from the TSV reports the earlier phases produced —
Markdown by default, `--format html` for HTML. It reads reports from `report/`
(`-r/--report`) and the markdup `.dups.tsv` from `output/` (`-o/--out`), and
writes one page per stem to `summary/` (`-d/--dest`). A stem with no report files
(e.g. the not-yet-published Baltic GL product) is skipped, not an error.

```bash
scripts/summary_data.sh                       # summary/<stem>.md for every unit
scripts/summary_data.sh -y -f html baltic     # HTML pages, Baltic only, no prompt
```

The script gives each page a human-readable **title** and any product-specific
**notes** (passed to `report summary` as `--title` / `--note`). Both live in the
*Page text* section near the top of the script — `title_for` and `notes_for`, one
`case` arm per stem — and that is the place to edit what a page says about a
region or dataset. The section prose on the page is generic and comes from
`ctddump` itself.

## `summary_site.sh`

Renders the Markdown pages from `summary_data.sh` into a **static local web
site** with [mdBook](https://rust-lang.github.io/mdBook/). Pages are read from
`summary/` (`-s/--src`) and the built site is written to `site/` (`-d/--dest`);
open `site/index.html` in a browser, or serve the directory as-is.

Chapters are grouped into one part per region, in the same order the other
scripts use. Each chapter's name comes from the page's own top-level heading (the
title `summary_data.sh` gave it), so titles are defined in exactly one place. A
region whose pages are absent is skipped rather than left as an empty part; if no
pages are found at all, that is an error rather than an empty site.

The book is assembled in a temporary directory, so only the rendered site is left
behind. This phase needs the Markdown pages, so run `summary_data.sh` with its
default `-f md` (an HTML run has nothing for mdBook to render).

The built directory is made self-contained and ready to publish: mdBook writes a
`.nojekyll` (so GitHub Pages serves its `FontAwesome/` and other asset folders),
and the script adds a `LICENSE` (the project's own by default, `-l/--license FILE`
to override or `--license ""` to skip) and a short `README.md` describing the
site. This is what makes the output directory suitable to push straight to a
publishing repository such as `ctddump-report-example`.

A live example of a built site is published at
<https://aiqc-hub.github.io/ctddump-report-example/> (a static, point-in-time
sample of this phase's output).

```bash
scripts/summary_site.sh                      # site/ from summary/, every region
scripts/summary_site.sh -y arctic            # Arctic pages only, no prompt
scripts/summary_site.sh -t "Arctic CTD QC"   # override the book title
```

By default the script writes a `book.toml` for you. Pass `-c/--config FILE` to use
your own instead; it is used verbatim, so it must keep mdBook's default
`src = "src"` — the script assembles the chapters into a `src/` directory beside
it. With `--config`, `--title` is ignored (your file sets the title).

```bash
scripts/summary_site.sh -c my-book.toml -y all
```

## Full pipeline

Run the six phases in order (skipping prompts) for every region. Log in once
first if you have not already (`scripts/download_data.sh login`):

```bash
scripts/download_data.sh -y            # download every region into source/
scripts/convert_data.sh  -y all
scripts/clean_data.sh    -y all
scripts/dedup_data.sh    -y all
scripts/summary_data.sh  -y all
scripts/summary_site.sh  -y all        # site/index.html
```

For the equivalent commands spelled out step by step, see the
[Regional workflows](./examples/arctic.md).
