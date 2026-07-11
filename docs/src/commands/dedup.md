# `dedup`

Remove duplicate profiles from a produced **Parquet** data file, keeping one
profile per duplicate group.

```
ctddump dedup [OPTIONS] <src.parquet> <dest.parquet>
```

`dedup` re-derives the **same duplicate key** as [`markdup`](./markdup.md) — from
`profile_timestamp` (date only by default) and `longitude`/`latitude` (3-decimal
rounding by default) — so it can run standalone with the same options. Within
each group of profiles sharing a key, it keeps the profile with the **most
observation rows**; ties are broken by **first appearance**. Profiles with no key
(NaN position or null timestamp) are always kept.

If the input has an `is_dup` column (from `markdup`), it is reset to `false` —
the survivors are unique — so the schema stays stable across the
`markdup → dedup` pipeline and a follow-up report shows zero duplicates.

Two streaming passes bound memory regardless of file size.

## Options

The key options match `markdup` and should be given the **same** values used
there:

| Option | Default | Meaning |
|--------|---------|---------|
| `--time-format <FMT>` | `%Y-%m-%d` | strftime format applied to `profile_timestamp` for the key |
| `--decimals <N>` | `3` | decimal places `longitude`/`latitude` are rounded to |
| `--round-mode <MODE>` | `round` | `round`, `floor`, `ceil`, or `trunc` |

```bash
# Drop duplicates using the default key (same date + 3-dp coordinates)
ctddump dedup marked.parquet deduped.parquet
```
