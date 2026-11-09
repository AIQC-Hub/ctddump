# Output schema

Every converter produces the **same** observation-level flat table, so files
from different sources can be merged directly.

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | see [Row order and numbering](#row-order-and-numbering) |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | see [Row order and numbering](#row-order-and-numbering) |
| `longitude` / `latitude` | `f32` (NRT) / `f64` (CORA) | `generic`: as NRT for `time-depth`, as CORA for `profile-level` |
| `profile_longitude` / `profile_latitude` | `f32` (NRT) / `f64` (CORA) | from `PRECISE_*` or expanded `DEPLOY_*`; NaN when unavailable (always for `generic`) |
| `time_qc` / `position_qc` | `String` | `""` if absent |
| `filename` | `String` | source file stem |
| `temp`, `psal`, `pres`, `deph` | `f32` | NaN where missing |
| `temp_qc`, `psal_qc`, `pres_qc`, `deph_qc` | `String` | single-char flag; `""` if missing |
| `temp_dm`, `psal_dm`, `pres_dm` | `String` | data mode: `R`, `A`, or `D`; `""` when unknown (see below) |
| `pres_conv`, `deph_conv` | `i8` | `1` = value derived by conversion |

Pressure and depth are converted to each other with the TEOS-10 (`gsw`)
routines when only one is present, and the `*_conv` columns flag which values
were derived rather than measured.

## Row order and numbering

**`concat convert` sorts each profile's observations by increasing pressure.**
A single `convert` (or `batch convert`) does not: it keeps the order of the
source file. Which order a Parquet file has depends on which step wrote it.

| Step | Observation order | `profile_no` | `observation_no` |
|------|-------------------|--------------|------------------|
| `convert`, `batch convert` | as in the source file (its level dimension) | the profile's index in the file, from 1 | the level's index in the file, from 1; levels with no valid measurement are dropped, leaving gaps |
| `concat convert` *(default)* | **increasing `pres`** within each profile | renumbered from 1 within each platform | renumbered `1..N` within each profile, following increasing `pres` |
| `concat convert --no-pres-sort` | as in the source files | renumbered | renumbered `1..N`, following the source order |
| `concat convert --no-renumber` | as read, unsorted | kept from the input | kept from the input |

Details of the default `concat convert`:

- Rows are sorted by `platform_code, profile_timestamp, longitude, latitude,
  pres`. Profiles are therefore in time order within each platform, and the
  observations of a profile run from the surface down.
- Rows whose `pres` is missing are dropped first, so a profile with no
  pressure at all disappears here. With `--keep-na-pres` they are kept and
  sorted to the end of their profile, after the valid pressures.
- Observations with the same pressure keep their source order (by file path,
  then by position in the file).
- `profile_no` is assigned per distinct `(platform_code, profile_timestamp,
  longitude, latitude)`. Two profiles from different files that share all four
  become one profile, and the pressure sort interleaves their observations.
  `filename` still tells them apart.
- All other columns move with their row; nothing is interpolated or averaged.

The pipeline scripts and the regional workflows merge with the default, so
their merged Parquet files, and everything cleaned, de-duplicated, or
summarised from them, are sorted by pressure. Pass `--no-pres-sort` to keep the
acquisition order instead (see [`concat`](./commands/concat.md)). To find the
profiles whose source order is not increasing in pressure, run
[`checkpres`](./commands/checkpres.md) on the per-file Parquet before the merge.

## Data mode

The `*_dm` columns record how each value was processed:

| Code | Meaning |
|------|---------|
| `R` | real-time |
| `A` | adjusted in real time |
| `D` | delayed mode |

The code is copied from the source file; ctddump never infers it. Where it
comes from depends on the source:

- **NRT, with a `<PARAM>_DM` variable** (`TEMP_DM`, `PSAL_DM`, `PRES_DM`; found
  in some Arctic files, whose parameters then carry `data_mode = "M"` for
  "mixed"): the per-observation code is copied through. A fill cell becomes `""`.
- **NRT, without one**: every observation gets the parameter's `data_mode`
  attribute when it is `R`, `A`, or `D`. Any other attribute, notably `"M"`,
  says the mode varies per observation without saying how, so the column is
  `""`; a missing attribute gives `""` too.
- **NRT pressure derived from depth** (`pres_conv = 1`): `pres_dm` is taken from
  `DEPH_DM` (or DEPH's `data_mode` attribute), the same way `pres_qc` takes
  `deph_qc`.
- **CORA**: there are no per-parameter data-mode variables, so all three columns
  hold the profile's `DATA_MODE`, repeated on each of its observations (`""` if
  the file has no `DATA_MODE`, or for a profile whose cell is a fill value).
- **`generic`**: as NRT for the `time-depth` layout and as CORA for
  `profile-level`, except that `[defaults] data_mode` replaces `""` when the
  file has no data mode at all (see
  [Your own NetCDF files](./your-own-netcdf.md#how-values-are-read)). A `"M"`
  attribute stays `""` even with a default.

A parameter the file does not have at all (all NaN) has `""` as well.

`convert` and `batch convert` print one warning per file whose data mode is
left empty for a whole variable (a `"M"` attribute without `<PARAM>_DM`, no
data-mode information at all, or a CORA file without `DATA_MODE`), naming the
variables. Fill cells inside a data-mode variable are not reported.

Keep in mind when using these columns:

- `""` means "no code in the file", not "no measurement": a valid value can
  have an empty data mode, and a missing value can have a code.
- The values and flags are always the ones in `TEMP` / `TEMP_QC` (and so on);
  `*_ADJUSTED` variables are never read. In Copernicus NRT files the
  delayed-mode values and flags are in those variables, with `<PARAM>_DM`
  saying which observations they are.
- CORA's `DATA_MODE` comes from the original observation. CORA re-processes
  the QC of every profile, so `R` there does not mean the flags are real-time
  ones.
