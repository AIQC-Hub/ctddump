# `markdup`

Mark duplicate profiles in a produced **Parquet** data file with an `is_dup`
column, and write a TSV listing the duplicated profiles.

```
ctddump markdup [OPTIONS] <src.parquet> <dest.parquet> <dups.tsv>
```

Two profiles are **duplicates** when they share the same key, built from:

- **`profile_timestamp`**: formatted with a strftime string; **date only**
  (`%Y-%m-%d`) by default.
- **`longitude`** and **`latitude`**: rounded to **3 decimals** by default.

`platform_code` is deliberately **not** part of the key, so duplicates are
detected *across platforms*. A profile whose position is NaN or whose timestamp
is null has no key and is never marked.

The `is_dup` column (Boolean) is `true` for every profile whose key is shared by
at least one other profile. Two streaming passes bound memory regardless of file
size.

## Options

| Option | Default | Meaning |
|--------|---------|---------|
| `--time-format <FMT>` | `%Y-%m-%d` | strftime format applied to `profile_timestamp` for the key |
| `--decimals <N>` | `3` | decimal places `longitude`/`latitude` are rounded to |
| `--round-mode <MODE>` | `round` | `round`, `floor`, `ceil`, or `trunc` |

## Outputs

1. **`dest.parquet`**: the input plus the `is_dup` column.
2. **`dups.tsv`**: one row per duplicated profile, grouped by `dup_group`:
   `dup_group, platform_code, profile_no, profile_time, profile_timestamp,
   longitude, latitude, n_obs`.

```bash
# Default key: same date + coordinates rounded to 3 decimals
ctddump markdup merged.parquet marked.parquet duplicates.tsv

# Match on the full hour and 2-decimal coordinates
ctddump markdup --time-format "%Y-%m-%d %H" --decimals 2 merged.parquet marked.parquet dups.tsv
```

Use [`dedup`](./dedup.md) to then drop the duplicates, and
[`report parquet`](./report.md) to summarise the duplicate counts.
