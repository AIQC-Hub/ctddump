# Your own NetCDF files

`ctddump` is built around the Copernicus files, but it can also convert your own
NetCDF files, for example from a mooring, a glider campaign, or a research
cruise. Use the **`generic`** format with a small **mapping file** that tells
`ctddump` what your dimensions and variables are called. The output has the same
[schema](./output-schema.md) as the Copernicus conversions, so your data can be
merged, cleaned, de-duplicated, and reported on with the rest of the toolkit.

```bash
ctddump convert generic --config my_map.toml my_data.nc my_data.parquet
```

## What your file needs

Your file must be laid out like a Copernicus file: a **profile** dimension and a
**level** dimension, with the measurements stored as 2-D variables indexed
`[profile, level]`, in that order.

| Information | Where it can come from | Required? |
|-------------|------------------------|-----------|
| Profile and level dimensions | any two dimension names | yes |
| Time | a variable, scalar or `[profile]`, with CF `units` such as `"seconds since 1970-01-01"` | yes |
| Longitude, latitude | variables, scalar or `[profile]`, **or** a default value | yes |
| Platform code | a global attribute, a variable, **or** a default value | yes |
| Pressure **or** depth | a `[profile, level]` variable | at least one |
| Temperature **or** salinity | a `[profile, level]` variable | at least one |
| QC flags | `[profile, level]` variables (integer or char) | no: empty when absent |
| Data mode | variables, a `data_mode` attribute, **or** a default value | no: `R` when absent |

Two layouts are supported. They differ in what a profile is and where the
platform code usually lives:

```text
time-depth (default)                    profile-level
like Copernicus NRT                     like Copernicus CORA / Argo

  one platform per file                   many profiles, many platforms
  platform: global attribute              platform: [profile] variable

          level ->                                level ->
  profile [ x  x  x  x ]  TEMP[TIME,DEPTH]  profile [ x  x  x  . ]  TEMP[N_PROF,N_LEVELS]
     |    [ x  x  x  . ]                       |    [ x  x  .  . ]
     v    [ x  x  x  x ]                       v    [ x  x  x  x ]
  TIME[TIME]                               TIME[N_PROF], PLATFORM_NUMBER[N_PROF, ...]
```

Profiles are numbered in file order (`profile_no`), and levels within each
profile (`observation_no`). Cells where temperature, salinity, and pressure are
all missing are dropped.

## Example: a mooring

Suppose `ncdump -h mooring.nc` shows:

```text
dimensions:
    time = 8760 ;
    z = 12 ;
variables:
    double time(time) ;
        time:units = "seconds since 1970-01-01 00:00:00" ;
    float lat ;
    float temperature(time, z) ;
        temperature:_FillValue = -999.f ;
    short temperature_flag(time, z) ;
    short sal(time, z) ;
        sal:scale_factor = 0.01 ;
        sal:add_offset = 30. ;
    double pressure(time, z) ;
```

The file has no longitude and no platform code, so the mapping supplies both
as defaults:

```toml
# mooring.toml
[dimensions]
profile = "time"
level   = "z"

[variables]
time      = "time"
latitude  = "lat"
temp      = "temperature"
temp_qc   = "temperature_flag"
psal      = "sal"
pres      = "pressure"

[defaults]
platform_code = "MOORING_1"
longitude     = 20.0
data_mode     = "D"       # our own, quality-controlled data
```

```bash
ctddump convert generic --config mooring.toml mooring.nc mooring.parquet
ctddump report parquet mooring.parquet     # check the result
```

Time is converted from seconds since 1970 to the output's days since 1950; the
fill values become NaN; salinity is unpacked with `scale_factor` and
`add_offset`; and depth is derived from pressure (flagged with `deph_conv = 1`).

## Example: a cruise

A cruise file with many casts, each with its own ship code, uses the
`profile-level` layout:

```text
dimensions:
    cast = 54 ;
    level = 400 ;
    strlen = 8 ;
variables:
    char ship(cast, strlen) ;
    double time(cast) ;
        time:units = "days since 2000-01-01T00:00:00Z" ;
    double lon(cast) ;
    double lat(cast) ;
    float depth(cast, level) ;
    float TEMP(cast, level) ;
    char TEMP_QC(cast, level) ;
    float PSAL(cast, level) ;
    char PSAL_QC(cast, level) ;
```

```toml
# cruise.toml
layout = "profile-level"

[dimensions]
profile = "cast"
level   = "level"

[variables]
time      = "time"
longitude = "lon"
latitude  = "lat"
deph      = "depth"
# TEMP, TEMP_QC, PSAL and PSAL_QC already have the default names.

[platform]
variable = "ship"
```

Pressure is derived from depth here. The char QC flags are detected
automatically.

## Many files at once

`batch convert generic` converts a directory tree with the same mapping, and
`header generic` / `batch header generic` export the metadata (dimensions,
variables, and **all** global attributes). The results feed into the usual
pipeline:

```bash
ctddump batch convert generic --config cruise.toml --output output/convert source/cruises
ctddump batch header  generic --output output/header source/cruises
ctddump concat convert output/convert output/cruises.parquet
ctddump concat header  output/header  output/cruises.yaml
ctddump report parquet output/cruises.parquet
```

`batch` matches `*.nc` by default; set `--pattern` (or `pattern` in the mapping
file) to narrow it. The [helper scripts](./scripts.md) are written for the
Copernicus downloads and do not run the `generic` format.

## The mapping file

Every key is optional. A missing key takes the **Copernicus name**, so an empty
mapping file describes a Copernicus NRT file. Setting a variable name to `""`
says the file has no such variable. Misspelt keys are rejected rather than
ignored.

```toml
layout     = "time-depth"   # or "profile-level"
time_units = "..."          # overrides the time variable's units attribute
qc_type    = "int"          # or "char"; detected per variable when omitted
pattern    = "*.nc"         # batch file selection

[dimensions]
profile = "TIME"            # profile-level: N_PROF
level   = "DEPTH"           # profile-level: N_LEVELS

[variables]
time        = "TIME"
longitude   = "LONGITUDE"
latitude    = "LATITUDE"
time_qc     = "TIME_QC"
position_qc = "POSITION_QC"
temp        = "TEMP"        # temp_qc / temp_dm default to "<temp>_QC" / "<temp>_DM"
psal        = "PSAL"        # likewise psal_qc / psal_dm
pres        = "PRES"        # likewise pres_qc / pres_dm
deph        = "DEPH"        # likewise deph_qc / deph_dm
data_mode   = "DATA_MODE"   # per-profile data mode (profile-level layout)

[platform]                  # one of the two
attribute = "platform_code" # time-depth default: a global attribute
# variable = "PLATFORM_NUMBER"  # profile-level default: a per-profile variable

[defaults]                  # used when the file does not provide the value
platform_code = "MY_PLATFORM"
longitude     = 20.0
latitude      = 60.0
data_mode     = "D"         # R, A or D
```

A few flags override the mapping file, which helps when one mapping serves
several platforms:

| Flag | Mapping key |
|------|-------------|
| `--layout <time-depth\|profile-level>` | `layout` |
| `--time-units <UNITS>` | `time_units` |
| `--platform-code <CODE>` | `[defaults] platform_code` |
| `--pattern <GLOB>` | `pattern` |

The priority is: built-in default, then the `--config` file, then the flags.

## How values are read

- **Missing values.** `_FillValue` becomes NaN; without one, `missing_value`
  and then the NetCDF default fill value for the type are used. This applies to
  time and position as well as to the measurements.
- **Packed values.** `scale_factor` and `add_offset` are applied.
- **Time.** The units may be days, hours, minutes, seconds, or milliseconds
  since a reference date, with an optional time and time zone
  (`"hours since 2000-01-01 06:00 +01:00"`). The `calendar` must be `standard`,
  `gregorian`, or `proleptic_gregorian` (or absent). Months and years are not
  accepted because their length varies.
- **Platform code.** A variable may be `char [profile, strlen]`,
  `string [profile]`, an integer `[profile]` (e.g. a WMO number), or a scalar of
  any of these for the whole file. Blank entries take `[defaults] platform_code`.
- **QC flags.** Integer flags `0` to `9` are written as `"0"` to `"9"`, other
  values (fill values included) as `""`; any integer type works. Char flags are
  copied as they are, with blanks written as `""`.
- **Data mode.** In the `time-depth` layout, a `[profile, level]` char variable
  per parameter (`TEMP_DM`); in the `profile-level` layout, one per-profile
  variable (`DATA_MODE`). Without one, the parameter's `data_mode` attribute,
  then `[defaults] data_mode`, then `R`.
- **Pressure and depth.** Whichever is missing is derived from the other with
  TEOS-10 at the profile's latitude, and flagged in `pres_conv` / `deph_conv`.

## Limits and workarounds

| Your file | What to do |
|-----------|------------|
| A single cast with only a level dimension, `TEMP(depth)` | Add a profile dimension of length 1, e.g. with [NCO](https://nco.sourceforge.net/): `ncecat -O -u cast in.nc out.nc` |
| Measurements stored `[level, profile]` | Reorder the dimensions: `ncpdq -a cast,level in.nc out.nc` |
| Position or time per observation (a trajectory) | Not supported: the output has one position and one time per profile. |
| A profile name or ID variable | Not carried into the output. Profiles are identified by `platform_code`, `profile_no`, and time. |

## Troubleshooting

`ctddump` checks the file against the mapping before converting and names the
problem:

| Message | Fix |
|---------|-----|
| `dimension "depth" not found (the file has: ...)` | Set `[dimensions] profile` / `level` to names from the list. |
| `time variable "TIME" not found` | Set `[variables] time`. |
| `time variable "time" has no units attribute` | Set `time_units` (or pass `--time-units`). |
| `unsupported time units "..."` | Use a `"<unit> since <date>"` form; see *How values are read*. |
| `longitude variable "LONGITUDE" not found` | Set `[variables] longitude`, or `[defaults] longitude` for a fixed position. |
| `platform global attribute "platform_code" not found` | Set `[platform] attribute` / `variable`, or `[defaults] platform_code`. |
| `neither pressure "PRES" nor depth "DEPH" found` | Set `[variables] pres` or `deph`. |
| `variable "X" has dimensions [...]; expected [...]` | The variable has the wrong shape; see *Limits and workarounds*. |
| `unknown field ...` | A misspelt key in the mapping file. |

`ctddump header generic my_data.nc my_data.yaml` lists every dimension and
variable in a file, which helps when writing the mapping.
