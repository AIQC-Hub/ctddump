# Mediterranean Sea

An end-to-end workflow for the Mediterranean Sea in two phases: **data
preparation** (download, convert, merge, and export the metadata) and **data
cleaning** (drop low-quality profiles and restrict the data to the region).

## Data preparation

### Prerequisites

Downloading requires a free [Copernicus Marine](https://marine.copernicus.eu/)
account and the **Copernicus Marine Toolbox**
([documentation](https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox)),
which provides the `copernicusmarine` command used below.

Run everything from a working directory (e.g. `ctddump`). Downloads land under
`input/`; ctddump writes its results under `output/` (both created as needed).
Create `input`, enter it, and log in once:

```shell
mkdir input
cd input
copernicusmarine login
```

### 1. Download the data

```shell
# NRT — Mediterranean (MO) and Global (GL)
copernicusmarine get -i cmems_obs-ins_med_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Mediterranean
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "mediterrane/*/*_PR_CT.nc"

# Back to the working root; the steps below use input/ and output/ relative to it.
cd ..
```

### 2. Convert NetCDF to Parquet

```shell
# NRT MO
ctddump batch convert nrt_mo --threads 10 --output output/parquet/mo/mo input

# NRT GL
ctddump batch convert nrt_gl --threads 10 --output output/parquet/mo/gl input/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch convert cora --threads 10 --output output/parquet/mo/cora input/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

### 3. Merge the Parquet files

```shell
# NRT MO
ctddump concat convert --threads 10 output/parquet/mo/mo output/parquet/nrt_mo_mo.parquet

# NRT GL
ctddump concat convert --threads 10 output/parquet/mo/gl output/parquet/nrt_mo_gl.parquet

# CORA MO
ctddump concat convert --threads 10 output/parquet/mo/cora output/parquet/cora_mo.parquet
```

### 4. Export the metadata (headers)

```shell
# NRT MO
ctddump batch header nrt --threads 10 --pattern "MO_PR_CT_*.nc" --output output/header/mo/mo input/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# NRT GL
ctddump batch header nrt --threads 10 --pattern "GL_PR_CT_*.nc" --output output/header/mo/gl input/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch header cora --threads 10 --output output/header/mo/cora input/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

### 5. Merge the header files

```shell
# NRT MO
ctddump concat header output/header/mo/mo output/header/nrt_mo_mo.yaml

# NRT GL
ctddump concat header output/header/mo/gl output/header/nrt_mo_gl.yaml

# CORA MO
ctddump concat header output/header/mo/cora output/header/cora_mo.yaml
```

### 6. Summarise the results

Write a platform-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
mkdir -p output/report/convert

# NRT MO
ctddump report parquet --level platform output/parquet/nrt_mo_mo.parquet output/report/convert/nrt_mo_mo.parquet.tsv
ctddump report yaml output/header/nrt_mo_mo.yaml output/report/convert/nrt_mo_mo.yaml.tsv

# NRT GL
ctddump report parquet --level platform output/parquet/nrt_mo_gl.parquet output/report/convert/nrt_mo_gl.parquet.tsv
ctddump report yaml output/header/nrt_mo_gl.yaml output/report/convert/nrt_mo_gl.yaml.tsv

# CORA MO
ctddump report parquet --level platform output/parquet/cora_mo.parquet output/report/convert/cora_mo.parquet.tsv
ctddump report yaml output/header/cora_mo.yaml output/report/convert/cora_mo.yaml.tsv
```

## Data cleaning

Clean the merged Parquet from the preparation phase by dropping low-quality
profiles and restricting the data to the region. Each step reads the previous
step's output, so the stages chain `dropqc → dropna → filter`.

Create the output directories:

```shell
mkdir -p output/clean/dropqc output/clean/dropna output/clean/filter output/report/clean
```

### 1. Drop profiles with bad profile-level QC

Drop profiles whose `time_qc` or `position_qc` is a present, non-OK flag;
profiles that are OK (`"1"`) or have missing QC are kept.

```shell
# NRT MO
ctddump dropqc output/parquet/nrt_mo_mo.parquet output/clean/dropqc/nrt_mo_mo.parquet

# NRT GL
ctddump dropqc output/parquet/nrt_mo_gl.parquet output/clean/dropqc/nrt_mo_gl.parquet

# CORA MO
ctddump dropqc output/parquet/cora_mo.parquet output/clean/dropqc/cora_mo.parquet
```

### 2. Drop profiles with no usable data

Drop profiles that are entirely NA in any of `temp`, `psal`, or `pres`.

```shell
# NRT MO
ctddump dropna output/clean/dropqc/nrt_mo_mo.parquet output/clean/dropna/nrt_mo_mo.parquet

# NRT GL
ctddump dropna output/clean/dropqc/nrt_mo_gl.parquet output/clean/dropna/nrt_mo_gl.parquet

# CORA MO
ctddump dropna output/clean/dropqc/cora_mo.parquet output/clean/dropna/cora_mo.parquet
```

### 3. Filter to the Mediterranean region

Keep profiles inside the Mediterranean bounding box (longitude -5.61 to 35.567,
latitude 28.378 to 45.755), then exclude two sub-boxes: (longitude 27 to 36,
latitude 41 to 46) and (longitude -5.61 to 0, latitude 42 to 46). Each stage
chains through an intermediate `.box*.parquet` file to produce the final cleaned
file.

```shell
# NRT MO
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 output/clean/dropna/nrt_mo_mo.parquet output/clean/filter/nrt_mo_mo.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 output/clean/filter/nrt_mo_mo.box1.parquet output/clean/filter/nrt_mo_mo.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 output/clean/filter/nrt_mo_mo.box2.parquet output/clean/filter/nrt_mo_mo.parquet

# NRT GL
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 output/clean/dropna/nrt_mo_gl.parquet output/clean/filter/nrt_mo_gl.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 output/clean/filter/nrt_mo_gl.box1.parquet output/clean/filter/nrt_mo_gl.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 output/clean/filter/nrt_mo_gl.box2.parquet output/clean/filter/nrt_mo_gl.parquet

# CORA MO
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 output/clean/dropna/cora_mo.parquet output/clean/filter/cora_mo.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 output/clean/filter/cora_mo.box1.parquet output/clean/filter/cora_mo.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 output/clean/filter/cora_mo.box2.parquet output/clean/filter/cora_mo.parquet
```

### 4. Summarise the cleaned data

```shell
# NRT MO
ctddump report parquet --level platform output/clean/filter/nrt_mo_mo.parquet output/report/clean/nrt_mo_mo.parquet.tsv

# NRT GL
ctddump report parquet --level platform output/clean/filter/nrt_mo_gl.parquet output/report/clean/nrt_mo_gl.parquet.tsv

# CORA MO
ctddump report parquet --level platform output/clean/filter/cora_mo.parquet output/report/clean/cora_mo.parquet.tsv
```

## Data de-duplication

De-duplicate the cleaned Parquet from the previous phase. Two profiles are
duplicates when they share the same date and position (longitude/latitude rounded
to 3 decimals) — ctddump's defaults, across platforms. `markdup` flags them (and
lists them in a TSV); `dedup` removes them, keeping the profile with the most
observations.

Create the output directories:

```shell
mkdir -p output/dedup/markdup output/dedup/dedup output/report/dedup/markdup output/report/dedup/dedup
```

### 1. Mark duplicate profiles

```shell
# NRT MO
ctddump markdup output/clean/filter/nrt_mo_mo.parquet output/dedup/markdup/nrt_mo_mo.parquet output/dedup/markdup/nrt_mo_mo.dups.tsv

# NRT GL
ctddump markdup output/clean/filter/nrt_mo_gl.parquet output/dedup/markdup/nrt_mo_gl.parquet output/dedup/markdup/nrt_mo_gl.dups.tsv

# CORA MO
ctddump markdup output/clean/filter/cora_mo.parquet output/dedup/markdup/cora_mo.parquet output/dedup/markdup/cora_mo.dups.tsv
```

### 2. Summarise the marked data (duplicate counts)

```shell
# NRT MO
ctddump report parquet --level platform output/dedup/markdup/nrt_mo_mo.parquet output/report/dedup/markdup/nrt_mo_mo.parquet.tsv

# NRT GL
ctddump report parquet --level platform output/dedup/markdup/nrt_mo_gl.parquet output/report/dedup/markdup/nrt_mo_gl.parquet.tsv

# CORA MO
ctddump report parquet --level platform output/dedup/markdup/cora_mo.parquet output/report/dedup/markdup/cora_mo.parquet.tsv
```

### 3. Remove duplicate profiles

```shell
# NRT MO
ctddump dedup output/dedup/markdup/nrt_mo_mo.parquet output/dedup/dedup/nrt_mo_mo.parquet

# NRT GL
ctddump dedup output/dedup/markdup/nrt_mo_gl.parquet output/dedup/dedup/nrt_mo_gl.parquet

# CORA MO
ctddump dedup output/dedup/markdup/cora_mo.parquet output/dedup/dedup/cora_mo.parquet
```

### 4. Summarise the de-duplicated data

```shell
# NRT MO
ctddump report parquet --level platform output/dedup/dedup/nrt_mo_mo.parquet output/report/dedup/dedup/nrt_mo_mo.parquet.tsv

# NRT GL
ctddump report parquet --level platform output/dedup/dedup/nrt_mo_gl.parquet output/report/dedup/dedup/nrt_mo_gl.parquet.tsv

# CORA MO
ctddump report parquet --level platform output/dedup/dedup/cora_mo.parquet output/report/dedup/dedup/cora_mo.parquet.tsv
```

> The pipeline is automated by
> [`scripts/download_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/download_data.sh),
> [`scripts/convert_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/convert_data.sh),
> [`scripts/clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh),
> and [`scripts/dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh)
> — see [Helper scripts](../scripts.md) for their commands and options.
