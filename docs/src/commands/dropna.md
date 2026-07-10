# `dropna`

Filter a produced **Parquet** data file, dropping whole profiles that carry no
usable data in one of the core measurement parameters, and write the survivors
to a new Parquet file.

```
ctddump dropna <src.parquet> <dest.parquet>
```

A profile is **kept** only if **each** of `temp`, `psal`, and `pres` has at
least one non-NA observation. It is **dropped** if **any one** of those
parameters is entirely NA (null or NaN) across the profile. Partial NAs within a
parameter are fine — a kept profile retains *all* of its observations, including
the NA ones.

| Profile | `temp` | `psal` | `pres` | Result |
|---------|--------|--------|--------|--------|
| some valid, some NaN | ✅ has data | ✅ has data | ✅ has data | **kept** |
| all NaN in one param | ✅ | ❌ all NaN | ✅ | **dropped** |

The file is processed in two streaming passes (build the keep-set, then re-emit
the kept rows), so peak memory stays bounded regardless of file size and the
result is independent of how the file is chunked (tune with `CTDDUMP_CHUNK_ROWS`,
as for `convert`).

```bash
# Drop profiles with an all-NA temp, psal, or pres
ctddump dropna merged.parquet cleaned.parquet
```
