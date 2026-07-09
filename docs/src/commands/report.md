# `report`

Summarise a produced **Parquet** data file or **YAML** header file as a
text report — TSV (default), plain text, or JSON.

```
ctddump report parquet [--level global|platform|profile] [--format tsv|text|json] <src.parquet> [dest]
ctddump report yaml    [--format tsv|text|json] <src.yaml> [dest]
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

- `n_obs` — number of observations.
- `time_qc_good` / `position_qc_good` — number of **profiles** whose flag is
  `"1"` (CMEMS "good"). These flags are per-profile; at `--level profile` the
  single flag value is shown instead.
- `na_temp` / `na_psal` / `na_pres` — count of missing (null/NaN) values.
- `{temp,psal,pres}_{min,max,mean}` — statistics over the valid (non-NaN)
  values. (Median is intentionally not computed.)
- `{longitude,latitude}_{min,max}` — geographic bounding box over the valid
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
| `has_temp`, `has_psal`, `has_pres`, `has_deph`, `has_time`, `has_position` | presence of each core column |
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
