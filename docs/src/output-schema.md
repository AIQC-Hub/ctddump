# Output schema

Every converter produces the **same** observation-level flat table, so files
from different sources can be merged directly.

| Column | Type | Notes |
|--------|------|-------|
| `platform_code` | `String` | |
| `profile_no` | `u32` | |
| `profile_time` | `f64` | days since 1950-01-01 |
| `profile_timestamp` | `Datetime(ms)` | Unix milliseconds |
| `observation_no` | `u32` | |
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
