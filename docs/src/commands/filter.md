# `filter`

Filter a produced **Parquet** data file by a geographic **bounding box**,
keeping or dropping whole profiles, and write the result to a new Parquet file.

```
ctddump filter [--mode include|exclude] \
  --min-lon <W> --max-lon <E> --min-lat <S> --max-lat <N> \
  <src.parquet> <dest.parquet>
```

The box is **inclusive** on all four edges, and `--min-*` must not exceed the
matching `--max-*` (an inverted box is rejected; antimeridian wrap is not
supported).

Because a profile's `longitude`/`latitude` are constant across its observations,
the box acts on **whole profiles** — every observation of a matching profile is
kept or dropped together.

`--mode` (default `include`):

| Mode | Effect |
|------|--------|
| `include` | keep only profiles **inside** the box |
| `exclude` | drop profiles inside the box, keep everything else |

A profile whose position is **NaN** (unknown) is treated as *outside* the box:
dropped by `include`, kept by `exclude`.

The file is streamed one row group at a time, so peak memory stays bounded
regardless of file size (tune with `CTDDUMP_CHUNK_ROWS`, as for `convert`).

```bash
# Keep only profiles inside a Mediterranean sub-box
ctddump filter \
  --min-lon 5 --max-lon 15 --min-lat 35 --max-lat 40 \
  merged.parquet med_box.parquet

# Remove profiles inside that box, keep the rest
ctddump filter --mode exclude \
  --min-lon 5 --max-lon 15 --min-lat 35 --max-lat 40 \
  merged.parquet outside_box.parquet
```
