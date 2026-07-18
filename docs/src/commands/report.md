# `report`

Summarise a produced **Parquet** data file or **YAML** header file as a
text report, TSV (default), plain text, or JSON, or assemble a multi-section
**summary page** (`report summary`) from the per-stage TSV reports.

```
ctddump report parquet [--level global|platform|profile] [--format tsv|text|json] <src.parquet> [dest]
ctddump report yaml    [--format tsv|text|json] <src.yaml> [dest]
ctddump report summary [--report-dir report] [--out-dir output] [--format md|html]
                       [--title TEXT] [--note TEXT]... [-o dest] <stem>
```

The report is written to `dest`, or to **stdout** when `dest` is omitted (so it
pipes cleanly into other tools).

## `report parquet`

Aggregates a data file at one of three levels (`--level`, default `platform`):

| Level | One row per | Identifier columns |
|-------|-------------|--------------------|
| `global` | the whole file | `n_platforms`, `n_profiles` |
| `platform` | `platform_code` | `platform_code`, `n_profiles` |
| `profile` | `(platform_code, profile_no)` | `platform_code`, `profile_no`, `profile_timestamp`, `longitude`, `latitude` |

Every row also reports:

- `n_obs`: number of observations.
- `time_qc_good` / `position_qc_good`: number of **profiles** whose flag is
  `"1"` (CMEMS "good"). These flags are per-profile; at `--level profile` the
  single flag value is shown instead.
- `na_temp` / `na_psal` / `na_pres`: count of missing (null/NaN) values.
- `{temp,psal,pres}_{min,max,mean}`: statistics over the valid (non-NaN)
  values. (Median is intentionally not computed.)
- `{longitude,latitude}_{min,max}`: geographic bounding box over the valid
  positions (`global` and `platform` levels only; at `profile` level the single
  `longitude`/`latitude` is already shown).

```bash
# Per-platform summary
ctddump report parquet --level platform merged.parquet report.tsv

# Whole-file summary, human-readable, to stdout
ctddump report parquet --level global --format text merged.parquet
```

## `report yaml`

Summarises a merged header YAML (as produced by [`concat header`](./concat.md)):
one row per source file.

| Column | Meaning |
|--------|---------|
| `filename` | source file stem |
| `has_temp`, `has_psal`, `has_pres`, `has_deph` | presence of each core measurement variable |
| `has_time_qc`, `has_position_qc` | presence of each profile-level QC flag (`TIME_QC` / `POSITION_QC`) |
| `extra_params` | `;`-joined list of the extra measurement parameters present |

`extra_params` is detected automatically: any `Float` variable dimensioned
`(TIME, DEPTH)` that is not a `_QC` flag and not a core physical
(`TEMP/PSAL/PRES/DEPH`). This captures biogeochemical/biological parameters
(`DOXY`, `FLU2`, `TUR3`, `CPHL`, `NTRA`, …) as well as other non-core
measurements (`CNDC`, `SVEL`, …) without a hard-coded list.

```bash
# YAML header summary as JSON
ctddump report yaml --format json merged.yaml report.json
```

## `report summary`

Assembles a single **Markdown** (default) or **HTML** (`--format html`) page for
one file *stem* (e.g. `nrt_ar_ar`) by reading the TSV reports the pipeline already
produced. It does not scan any Parquet itself, it aggregates the small per-stage
report TSVs, so it is instant.

Given a stem, the source files are **auto-located** at their standard pipeline
paths under `--report-dir` (default `report`) and `--out-dir` (default `output`):

| Section | Source file(s) |
|---------|----------------|
| File summary | `report/header/<stem>.yaml.tsv` |
| Conversion | `report/convert/<stem>.parquet.tsv` |
| Cleaning → Drop bad QC | `report/clean/dropqc/<stem>.parquet.tsv` |
| Cleaning → Drop all-NA profiles | `report/clean/dropna/<stem>.parquet.tsv` |
| Cleaning → Filter by region | `report/clean/filter/<stem>.parquet.tsv` |
| Deduplication → Mark duplicates | `report/dedup/markdup/<stem>.parquet.tsv` **and** `output/dedup/markdup/<stem>.dups.tsv` |
| Deduplication → Remove duplicates | `report/dedup/dedup/<stem>.parquet.tsv` |

Any section whose file is absent is **skipped**, so a partially-run pipeline still
produces a valid page (a parent heading appears only when it has at least one
present subsection). The Mark-duplicates section shows its within/across tables
only when the `.dups.tsv` is also present.

Each stage table reports **platforms / profiles / observations**. The
`% of original` and `Deleted` columns compare against the **Conversion** stage
(the baseline "original"); if Conversion is absent, the earliest present stage is
used.

The two **Deduplication** stages ran on the cleaned data, not on the original, so
their tables carry three further columns, `Cleaned`, `% of cleaned` and
`Deleted (cleaned)`, comparing them against the **last cleaning stage present**
(Filter, else Drop all-NA, else Drop bad QC). When no cleaning stage ran there is
nothing to compare against and the columns are omitted.

The **Filter by region** section carries a second table, **Bounding box**, with the
minimum and maximum longitude and latitude of the profiles that survived the
filter, in decimal degrees to three places. The extremes are aggregated from the
per-platform `longitude_min` / `longitude_max` / `latitude_min` / `latitude_max`
columns of the stage TSV, so they span every platform in the file. When the
Conversion report is present a second column, `Original`, gives the same extremes
for the converted data before any cleaning ran, which shows how far the filter
tightened the box. Profiles with a missing position are ignored, and if no profile
has a valid position the table is omitted entirely.

The Mark-duplicates section additionally lists the duplicate profiles that `dedup`
would remove, split into:

- **within a platform**: a `dup_group` confined to a single `platform_code`;
- **across platforms**: a group spanning two or more.

(A `dup_group` is a set of profiles sharing the duplicate key, date + rounded
position, so an across-platform group is the same cast reported by more than one
platform.)

A third table, **Duplicate group sizes**, shows how many profiles the groups hold:
one row per distinct size up to 10, then a single `11+` row for the tail.

### Title and notes

Each section carries a short, generic explanation of what the stage did, the same
prose for every region and dataset. Anything region- or product-specific goes in:

- `--title TEXT`: replaces the default `Summary: <stem>` heading.
- `--note TEXT`: a note rendered under the title; repeat for several notes.

Both are plain text and are escaped in HTML output. In the pipeline these are
supplied per stem by [`summary_data.sh`](../scripts.md), which is the place to edit
what a page says about a given region or product.

```bash
# Markdown page for the Arctic AR stem, to stdout
ctddump report summary nrt_ar_ar

# HTML page, with a title, notes, and non-default roots, to a file
ctddump report summary nrt_bo_bo --report-dir report --out-dir output \
  --format html --title "Baltic Sea: Near Real Time, regional product (BO)" \
  --note "Each source file holds a single platform." \
  -o summary/nrt_bo_bo.html
```
