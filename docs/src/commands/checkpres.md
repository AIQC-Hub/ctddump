# `checkpres`

Find profiles whose **pressure does not increase** in observation order, and
write three TSV reports: the profiles, their platforms, and totals.

```
ctddump checkpres [OPTIONS] <src> <out_prefix>
```

`<src>` is a directory of Parquet files, read the same way as by
[`concat convert`](./concat.md), or a single Parquet file.

## When to run it

Run it on the per-file Parquet written by [`batch convert`](./batch.md),
**before** [`concat convert`](./concat.md) merges them. The merge sorts every
profile by increasing pressure (see
[Row order and numbering](../output-schema.md#row-order-and-numbering)), so
afterwards the original order can no longer be checked. Before the merge, rows
are still in the source file's order and `observation_no` is the level index in
the file.

Run it once per merge, on the same directory, so the reports cover exactly what
ends up in one merged file:

```bash
ctddump checkpres output/convert/ar/ar report/convert/checkpres/nrt_ar_ar
ctddump concat convert output/convert/ar/ar output/convert/nrt_ar_ar.parquet
```

## What is checked

- A **profile** is one `(filename, platform_code, profile_no)`. A file may hold
  many profiles and many platforms; each profile is checked on its own.
- Within a profile, each observation that has a pressure is compared with the
  previous observation that had one. Missing pressures are skipped.
- A **decrease** is a step to a lower pressure. A repeated pressure is not a
  decrease.
- A profile with no usable pressure at all has nothing to check. It is left out
  of the counts and reported only as `n_no_pres`, so the counts match the
  profiles `concat convert` keeps (it drops observations without pressure).
- A profile with at least one decrease is reported, as one of two kinds:

| Kind | Meaning |
|------|---------|
| `reversed` | Pressure never goes up: the profile is stored from the bottom up. Sorting by pressure turns it the right way round. |
| `non_monotonic` | Pressure goes both up and down: loops, spikes, or levels stored out of order. These are the profiles to look at. |

## Options

| Option | Default | Meaning |
|--------|---------|---------|
| `--pattern <GLOB>` | `*.parquet` | Filename glob used when `<src>` is a directory |
| `--min-drop <DBAR>` | `0` | Ignore decreases no larger than this, for sensor noise |
| `--pres-qc <FLAGS>` | every pressure | Use only pressures whose `pres_qc` is one of these flags, e.g. `1,2` |
| `-t, --threads <N>` | all cores | Number of files checked in parallel; the reports are the same for any value |

## Outputs

Three tab-separated files, named after `<out_prefix>`:

1. **`<out_prefix>.profiles.tsv`**: one row per reported profile, sorted by
   platform and time.

   | Column | Meaning |
   |--------|---------|
   | `filename`, `platform_code`, `profile_no` | the profile, as in the per-file Parquet |
   | `profile_timestamp`, `profile_time`, `longitude`, `latitude` | when and where; with `platform_code`, these also find the profile after the merge |
   | `kind` | `reversed` or `non_monotonic` |
   | `n_obs`, `n_pres` | observations, and observations with a pressure used by the check |
   | `n_decreases` | number of decreasing steps |
   | `first_decrease_obs` | `observation_no` where the pressure first drops |
   | `max_decrease` | largest drop, in dbar |
   | `pres_first`, `pres_last`, `pres_min`, `pres_max` | pressure range, in dbar |
   | `pres_conv` | `1` when some pressure in the profile was derived from depth |

2. **`<out_prefix>.platforms.tsv`**: one row per platform with at least one
   reported profile: `platform_code, n_files, n_profiles, n_bad_profiles,
   pct_bad, n_reversed, n_non_monotonic, first_date, last_date`. The dates are
   those of its first and last reported profile.

3. **`<out_prefix>.summary.tsv`**: one row for the whole source: `n_files,
   n_platforms, n_profiles, n_no_pres, n_bad_profiles, pct_bad, n_reversed,
   n_non_monotonic, n_bad_platforms, n_bad_files`. `n_profiles` and
   `n_platforms` count only what was checked; `n_no_pres` is the number of
   profiles without any usable pressure.

The output directory is created if needed. A one-line summary is printed to
standard error. Finding problems is not an error: the exit status is `0`. When
no file matches, or the directory does not exist, the command says so and
writes nothing.

## Examples

```bash
# Check a directory of converted files
ctddump checkpres ./parquet reports/arctic

# Ignore drops of 0.1 dbar or less, and pressures not flagged good
ctddump checkpres --min-drop 0.1 --pres-qc 1,2 ./parquet reports/arctic

# Check one file
ctddump checkpres AR_PR_CT_58KN.parquet reports/58kn
```

Files read one at a time, in fixed row windows (`CTDDUMP_CHUNK_ROWS`), so memory
follows the size of one file plus the reported profiles.
