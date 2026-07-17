# `dropqc`

Filter a produced **Parquet** data file, dropping whole profiles whose
**profile-level** quality-control flags mark them as bad, and write the
survivors to a new Parquet file.

```
ctddump dropqc <src.parquet> <dest.parquet>
```

A profile is **kept** only if **both** `time_qc` and `position_qc` are either
`"1"` (OK) or **missing**. A missing flag is one that is absent from the source
NetCDF, or stored as the NA byte `-128`, both render as the empty string `""`
in the Parquet output. Any other flag (`"0"`, `"2"`…`"9"`, or a non-numeric char
code) **drops** the whole profile.

Missing QC is treated as acceptable on purpose: several source files ship no
profile-level `position_qc` (or `time_qc`) at all, and those profiles must be
kept rather than discarded. Note that the Argo "missing value" flag `9` is a
*present* value, **not** the NA byte, so a `"9"` flag is dropped.

| `time_qc` | `position_qc` | Result |
|-----------|---------------|--------|
| `1` (OK) | `1` (OK) | **kept** |
| `1` (OK) | `""` (missing / NA) | **kept** |
| `""` (missing) | `""` (missing) | **kept** |
| `4` (bad) | `1` (OK) | **dropped** |
| `1` (OK) | `0` (present, not OK) | **dropped** |
| `9` (present) | `9` (present) | **dropped** |

Because `time_qc`/`position_qc` are constant within a profile, this is a plain
per-row predicate that keeps or drops whole profiles. The file is streamed one
row group at a time (`set_parallel(false)`), so peak memory stays bounded
regardless of file size (tune with `CTDDUMP_CHUNK_ROWS`, as for `convert`).

```bash
# Drop profiles flagged bad in time_qc or position_qc
ctddump dropqc merged.parquet cleaned.parquet
```
