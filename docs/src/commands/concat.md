# `concat`

Merge many files from a directory tree into a **single** file.

```
ctddump concat convert [OPTIONS] <src_dir> <output_file>
ctddump concat header  [OPTIONS] <src_dir> <output_file>
```

- `concat convert` merges **Parquet** data files.
- `concat header` merges **YAML** metadata files.

## `concat convert`

By default the merge re-assigns `profile_no` and `observation_no` so that
profile numbers are unique and sequential within each platform. Its behaviour
is controlled by a few flags:

| Flag | Effect |
|------|--------|
| *(default)* | Renumber; sort by `platform_code, profile_timestamp, longitude, latitude, pres`; drop rows with missing `pres`; use all cores. |
| `--no-renumber` | Merge as-is, without re-assigning `profile_no` / `observation_no`. |
| `--no-pres-sort` | Sort without `pres`, keeping each profile's observations in their original source order instead of reordering by pressure. |
| `--keep-na-pres` | Keep rows whose `pres` is null/NaN (dropped by default). |
| `--threads N` | Cap the worker count (default: all cores). `--threads 1` is the sequential, lowest-memory path. |
| `--pattern <GLOB>` | Merge only files whose name matches the pattern. |

> Renumbering processes platform ranges in parallel via temporary files in the
> output folder. Higher thread counts are faster but use more memory — **the
> merged result is identical either way.**

### Examples

```bash
# Merge all Parquet files, with profile renumbering (the default)
ctddump concat convert ./parquet merged.parquet

# Merge without renumbering
ctddump concat convert --no-renumber ./parquet merged.parquet

# Keep each profile's observations in their original order
ctddump concat convert --no-pres-sort ./parquet merged.parquet

# Keep rows with missing pres
ctddump concat convert --keep-na-pres ./parquet merged.parquet

# Sequential, lowest-memory merge
ctddump concat convert --threads 1 ./parquet merged.parquet

# Merge only a subset
ctddump concat convert --pattern "AR_PR_CT_*.parquet" ./parquet merged.parquet
```

## `concat header`

Merges YAML header files — each file contributes its top-level keys to the
combined output. An error is raised if any two files share the same key.

```bash
ctddump concat header ./yaml merged.yaml
```
