# Arctic Sea

An end-to-end workflow for the Arctic Sea in two phases: **data preparation**
(download, convert, merge, and export the metadata) and **data cleaning** (drop
low-quality profiles and restrict the data to the region).

## Data preparation

### Prerequisites

Downloading requires a free [Copernicus Marine](https://marine.copernicus.eu/)
account and the **Copernicus Marine Toolbox**
([documentation](https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox)),
which provides the `copernicusmarine` command used below.

Run everything from a working directory (e.g. `ctddump`). Downloads land under
`source/`; ctddump writes data products under `output/` and summary
reports under `report/` (all created as needed). Create `source`, enter it, and
log in once:

```shell
mkdir source
cd source
copernicusmarine login
```

### 1. Download the data

```shell
# NRT: Arctic (AR) and Global (GL)
copernicusmarine get -i cmems_obs-ins_arc_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA: Arctic
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "arctic/*/*_PR_CT.nc"

# Back to the working root; the steps below use source/, output/, and report/ relative to it.
cd ..
```

### 2. Convert NetCDF to Parquet

```shell
# NRT AR
ctddump batch convert nrt_ar --threads 10 --output output/convert/ar/ar source

# NRT GL
ctddump batch convert nrt_gl --threads 10 --output output/convert/ar/gl source/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# CORA AR
ctddump batch convert cora --threads 10 --output output/convert/ar/cora source/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/arctic
```

### 3. Merge the Parquet files

```shell
# NRT AR
ctddump concat convert --threads 10 output/convert/ar/ar output/convert/nrt_ar_ar.parquet

# NRT GL
ctddump concat convert --threads 10 output/convert/ar/gl output/convert/nrt_ar_gl.parquet

# CORA AR
ctddump concat convert --threads 10 output/convert/ar/cora output/convert/cora_ar.parquet
```

### 4. Export the metadata (headers)

```shell
# NRT AR
ctddump batch header nrt --threads 10 --pattern "AR_PR_CT_*.nc" --output output/header/ar/ar source/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# NRT GL
ctddump batch header nrt --threads 10 --pattern "GL_PR_CT_*.nc" --output output/header/ar/gl source/INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031

# CORA AR
ctddump batch header cora --threads 10 --output output/header/ar/cora source/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/arctic
```

### 5. Merge the header files

```shell
# NRT AR
ctddump concat header output/header/ar/ar output/header/nrt_ar_ar.yaml

# NRT GL
ctddump concat header output/header/ar/gl output/header/nrt_ar_gl.yaml

# CORA AR
ctddump concat header output/header/ar/cora output/header/cora_ar.yaml
```

### 6. Summarise the results

Write a platform-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
mkdir -p report/convert report/header

# NRT AR
ctddump report parquet --level platform output/convert/nrt_ar_ar.parquet report/convert/nrt_ar_ar.parquet.tsv
ctddump report yaml output/header/nrt_ar_ar.yaml report/header/nrt_ar_ar.yaml.tsv

# NRT GL
ctddump report parquet --level platform output/convert/nrt_ar_gl.parquet report/convert/nrt_ar_gl.parquet.tsv
ctddump report yaml output/header/nrt_ar_gl.yaml report/header/nrt_ar_gl.yaml.tsv

# CORA AR
ctddump report parquet --level platform output/convert/cora_ar.parquet report/convert/cora_ar.parquet.tsv
ctddump report yaml output/header/cora_ar.yaml report/header/cora_ar.yaml.tsv
```

## Data cleaning

Clean the merged Parquet from the preparation phase by dropping low-quality
profiles and restricting the data to the region. Each step reads the previous
step's output, so the stages chain `dropqc → dropna → filter`.

Create the output directories:

```shell
mkdir -p output/clean/dropqc output/clean/dropna output/clean/filter \
         report/clean/dropqc report/clean/dropna report/clean/filter
```

### 1. Drop profiles with bad profile-level QC

Drop profiles whose `time_qc` or `position_qc` is a present, non-OK flag;
profiles that are OK (`"1"`) or have missing QC are kept.

```shell
# NRT AR
ctddump dropqc output/convert/nrt_ar_ar.parquet output/clean/dropqc/nrt_ar_ar.parquet

# NRT GL
ctddump dropqc output/convert/nrt_ar_gl.parquet output/clean/dropqc/nrt_ar_gl.parquet

# CORA AR
ctddump dropqc output/convert/cora_ar.parquet output/clean/dropqc/cora_ar.parquet
```

### 2. Drop profiles with no usable data

Drop profiles that are entirely NA in any of `temp`, `psal`, or `pres`.

```shell
# NRT AR
ctddump dropna output/clean/dropqc/nrt_ar_ar.parquet output/clean/dropna/nrt_ar_ar.parquet

# NRT GL
ctddump dropna output/clean/dropqc/nrt_ar_gl.parquet output/clean/dropna/nrt_ar_gl.parquet

# CORA AR
ctddump dropna output/clean/dropqc/cora_ar.parquet output/clean/dropna/cora_ar.parquet
```

### 3. Filter to the Arctic region

Keep only profiles inside the Arctic bounding box (longitude -180 to 180,
latitude 60 to 90).

```shell
# NRT AR
ctddump filter --min-lon -180 --max-lon 180 --min-lat 60 --max-lat 90 output/clean/dropna/nrt_ar_ar.parquet output/clean/filter/nrt_ar_ar.parquet

# NRT GL
ctddump filter --min-lon -180 --max-lon 180 --min-lat 60 --max-lat 90 output/clean/dropna/nrt_ar_gl.parquet output/clean/filter/nrt_ar_gl.parquet

# CORA AR
ctddump filter --min-lon -180 --max-lon 180 --min-lat 60 --max-lat 90 output/clean/dropna/cora_ar.parquet output/clean/filter/cora_ar.parquet
```

### 4. Summarise the cleaned data

Summarise each cleaning stage (as TSV), mirroring the data layout under
`report/clean/`.

```shell
# after dropqc
ctddump report parquet --level platform output/clean/dropqc/nrt_ar_ar.parquet report/clean/dropqc/nrt_ar_ar.parquet.tsv
ctddump report parquet --level platform output/clean/dropqc/nrt_ar_gl.parquet report/clean/dropqc/nrt_ar_gl.parquet.tsv
ctddump report parquet --level platform output/clean/dropqc/cora_ar.parquet   report/clean/dropqc/cora_ar.parquet.tsv

# after dropna
ctddump report parquet --level platform output/clean/dropna/nrt_ar_ar.parquet report/clean/dropna/nrt_ar_ar.parquet.tsv
ctddump report parquet --level platform output/clean/dropna/nrt_ar_gl.parquet report/clean/dropna/nrt_ar_gl.parquet.tsv
ctddump report parquet --level platform output/clean/dropna/cora_ar.parquet   report/clean/dropna/cora_ar.parquet.tsv

# after filter
ctddump report parquet --level platform output/clean/filter/nrt_ar_ar.parquet report/clean/filter/nrt_ar_ar.parquet.tsv
ctddump report parquet --level platform output/clean/filter/nrt_ar_gl.parquet report/clean/filter/nrt_ar_gl.parquet.tsv
ctddump report parquet --level platform output/clean/filter/cora_ar.parquet   report/clean/filter/cora_ar.parquet.tsv
```

## Data de-duplication

De-duplicate the cleaned Parquet from the previous phase. Two profiles are
duplicates when they share the same date and position (longitude/latitude rounded
to 3 decimals), ctddump's defaults, across platforms. `markdup` flags them (and
lists them in a TSV); `dedup` removes them, keeping the profile with the most
observations.

Create the output directories:

```shell
mkdir -p output/dedup/markdup output/dedup/dedup report/dedup/markdup report/dedup/dedup
```

### 1. Mark duplicate profiles

```shell
# NRT AR
ctddump markdup output/clean/filter/nrt_ar_ar.parquet output/dedup/markdup/nrt_ar_ar.parquet output/dedup/markdup/nrt_ar_ar.dups.tsv

# NRT GL
ctddump markdup output/clean/filter/nrt_ar_gl.parquet output/dedup/markdup/nrt_ar_gl.parquet output/dedup/markdup/nrt_ar_gl.dups.tsv

# CORA AR
ctddump markdup output/clean/filter/cora_ar.parquet output/dedup/markdup/cora_ar.parquet output/dedup/markdup/cora_ar.dups.tsv
```

### 2. Summarise the marked data (duplicate counts)

```shell
# NRT AR
ctddump report parquet --level platform output/dedup/markdup/nrt_ar_ar.parquet report/dedup/markdup/nrt_ar_ar.parquet.tsv

# NRT GL
ctddump report parquet --level platform output/dedup/markdup/nrt_ar_gl.parquet report/dedup/markdup/nrt_ar_gl.parquet.tsv

# CORA AR
ctddump report parquet --level platform output/dedup/markdup/cora_ar.parquet report/dedup/markdup/cora_ar.parquet.tsv
```

### 3. Remove duplicate profiles

```shell
# NRT AR
ctddump dedup output/dedup/markdup/nrt_ar_ar.parquet output/dedup/dedup/nrt_ar_ar.parquet

# NRT GL
ctddump dedup output/dedup/markdup/nrt_ar_gl.parquet output/dedup/dedup/nrt_ar_gl.parquet

# CORA AR
ctddump dedup output/dedup/markdup/cora_ar.parquet output/dedup/dedup/cora_ar.parquet
```

### 4. Summarise the de-duplicated data

```shell
# NRT AR
ctddump report parquet --level platform output/dedup/dedup/nrt_ar_ar.parquet report/dedup/dedup/nrt_ar_ar.parquet.tsv

# NRT GL
ctddump report parquet --level platform output/dedup/dedup/nrt_ar_gl.parquet report/dedup/dedup/nrt_ar_gl.parquet.tsv

# CORA AR
ctddump report parquet --level platform output/dedup/dedup/cora_ar.parquet report/dedup/dedup/cora_ar.parquet.tsv
```

> The pipeline is automated by
> [`scripts/download_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/download_data.sh),
> [`scripts/convert_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/convert_data.sh),
> [`scripts/clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh),
> and [`scripts/dedup_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/dedup_data.sh).
> See [Helper scripts](../scripts.md) for their commands and options.
