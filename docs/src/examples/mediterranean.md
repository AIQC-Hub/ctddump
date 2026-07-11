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

Create working directories and log in once:

```shell
mkdir copernicus parquet
cd copernicus
copernicusmarine login
```

### 1. Download the data

```shell
# NRT — Mediterranean (MO) and Global (GL)
copernicusmarine get -i cmems_obs-ins_med_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Mediterranean
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "mediterrane/*/*_PR_CT.nc"
```

### 2. Convert NetCDF to Parquet

```shell
# NRT MO
ctddump batch convert nrt_mo --threads 10 --output ../process_data/ctddump/parquet/mo/mo ../source_data/ctddump/netcdf

# NRT GL
ctddump batch convert nrt_gl --threads 10 --output ../process_data/ctddump/parquet/mo/gl ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch convert cora --threads 10 --output ../process_data/ctddump/parquet/mo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

### 3. Merge the Parquet files

```shell
# NRT MO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/mo ../process_data/ctddump/parquet/nrt_mo_mo.parquet

# NRT GL
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/gl ../process_data/ctddump/parquet/nrt_mo_gl.parquet

# CORA MO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/cora ../process_data/ctddump/parquet/cora_mo.parquet
```

### 4. Export the metadata (headers)

```shell
# NRT MO
ctddump batch header nrt --threads 10 --pattern "MO_PR_CT_*.nc" --output ../process_data/ctddump/header/mo/mo ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# NRT GL
ctddump batch header nrt --threads 10 --pattern "GL_PR_CT_*.nc" --output ../process_data/ctddump/header/mo/gl ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch header cora --threads 10 --output ../process_data/ctddump/header/mo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

### 5. Merge the header files

```shell
# NRT MO
ctddump concat header ../process_data/ctddump/header/mo/mo ../process_data/ctddump/header/nrt_mo_mo.yaml

# NRT GL
ctddump concat header ../process_data/ctddump/header/mo/gl ../process_data/ctddump/header/nrt_mo_gl.yaml

# CORA MO
ctddump concat header ../process_data/ctddump/header/mo/cora ../process_data/ctddump/header/cora_mo.yaml
```

### 6. Summarise the results

Write a platform-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
mkdir -p ../process_data/ctddump/report/prepare

# NRT MO
ctddump report parquet --level platform ../process_data/ctddump/parquet/nrt_mo_mo.parquet ../process_data/ctddump/report/prepare/nrt_mo_mo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_mo_mo.yaml ../process_data/ctddump/report/prepare/nrt_mo_mo.yaml.tsv

# NRT GL
ctddump report parquet --level platform ../process_data/ctddump/parquet/nrt_mo_gl.parquet ../process_data/ctddump/report/prepare/nrt_mo_gl.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_mo_gl.yaml ../process_data/ctddump/report/prepare/nrt_mo_gl.yaml.tsv

# CORA MO
ctddump report parquet --level platform ../process_data/ctddump/parquet/cora_mo.parquet ../process_data/ctddump/report/prepare/cora_mo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/cora_mo.yaml ../process_data/ctddump/report/prepare/cora_mo.yaml.tsv
```

## Data cleaning

Clean the merged Parquet from the preparation phase by dropping low-quality
profiles and restricting the data to the region. Each step reads the previous
step's output, so the stages chain `dropqc → dropna → filter`.

Create the output directories:

```shell
mkdir -p ../process_data/ctddump/clean/dropqc ../process_data/ctddump/clean/dropna ../process_data/ctddump/clean/filter ../process_data/ctddump/report/clean
```

### 1. Drop profiles with bad profile-level QC

Drop profiles whose `time_qc` or `position_qc` is a present, non-OK flag;
profiles that are OK (`"1"`) or have missing QC are kept.

```shell
# NRT MO
ctddump dropqc ../process_data/ctddump/parquet/nrt_mo_mo.parquet ../process_data/ctddump/clean/dropqc/nrt_mo_mo.parquet

# NRT GL
ctddump dropqc ../process_data/ctddump/parquet/nrt_mo_gl.parquet ../process_data/ctddump/clean/dropqc/nrt_mo_gl.parquet

# CORA MO
ctddump dropqc ../process_data/ctddump/parquet/cora_mo.parquet ../process_data/ctddump/clean/dropqc/cora_mo.parquet
```

### 2. Drop profiles with no usable data

Drop profiles that are entirely NA in any of `temp`, `psal`, or `pres`.

```shell
# NRT MO
ctddump dropna ../process_data/ctddump/clean/dropqc/nrt_mo_mo.parquet ../process_data/ctddump/clean/dropna/nrt_mo_mo.parquet

# NRT GL
ctddump dropna ../process_data/ctddump/clean/dropqc/nrt_mo_gl.parquet ../process_data/ctddump/clean/dropna/nrt_mo_gl.parquet

# CORA MO
ctddump dropna ../process_data/ctddump/clean/dropqc/cora_mo.parquet ../process_data/ctddump/clean/dropna/cora_mo.parquet
```

### 3. Filter to the Mediterranean region

Keep profiles inside the Mediterranean bounding box (longitude -5.61 to 35.567,
latitude 28.378 to 45.755), then exclude two sub-boxes: (longitude 27 to 36,
latitude 41 to 46) and (longitude -5.61 to 0, latitude 42 to 46). Each stage
chains through an intermediate `.box*.parquet` file to produce the final cleaned
file.

```shell
# NRT MO
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 ../process_data/ctddump/clean/dropna/nrt_mo_mo.parquet ../process_data/ctddump/clean/filter/nrt_mo_mo.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 ../process_data/ctddump/clean/filter/nrt_mo_mo.box1.parquet ../process_data/ctddump/clean/filter/nrt_mo_mo.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 ../process_data/ctddump/clean/filter/nrt_mo_mo.box2.parquet ../process_data/ctddump/clean/filter/nrt_mo_mo.parquet

# NRT GL
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 ../process_data/ctddump/clean/dropna/nrt_mo_gl.parquet ../process_data/ctddump/clean/filter/nrt_mo_gl.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 ../process_data/ctddump/clean/filter/nrt_mo_gl.box1.parquet ../process_data/ctddump/clean/filter/nrt_mo_gl.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 ../process_data/ctddump/clean/filter/nrt_mo_gl.box2.parquet ../process_data/ctddump/clean/filter/nrt_mo_gl.parquet

# CORA MO
ctddump filter --min-lon -5.61 --max-lon 35.567 --min-lat 28.378 --max-lat 45.755 ../process_data/ctddump/clean/dropna/cora_mo.parquet ../process_data/ctddump/clean/filter/cora_mo.box1.parquet
ctddump filter --mode exclude --min-lon 27 --max-lon 36 --min-lat 41 --max-lat 46 ../process_data/ctddump/clean/filter/cora_mo.box1.parquet ../process_data/ctddump/clean/filter/cora_mo.box2.parquet
ctddump filter --mode exclude --min-lon -5.61 --max-lon 0 --min-lat 42 --max-lat 46 ../process_data/ctddump/clean/filter/cora_mo.box2.parquet ../process_data/ctddump/clean/filter/cora_mo.parquet
```

### 4. Summarise the cleaned data

```shell
# NRT MO
ctddump report parquet --level platform ../process_data/ctddump/clean/filter/nrt_mo_mo.parquet ../process_data/ctddump/report/clean/nrt_mo_mo.parquet.tsv

# NRT GL
ctddump report parquet --level platform ../process_data/ctddump/clean/filter/nrt_mo_gl.parquet ../process_data/ctddump/report/clean/nrt_mo_gl.parquet.tsv

# CORA MO
ctddump report parquet --level platform ../process_data/ctddump/clean/filter/cora_mo.parquet ../process_data/ctddump/report/clean/cora_mo.parquet.tsv
```

> Both phases are automated by
> [`scripts/prepare_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/prepare_data.sh)
> and [`scripts/clean_data.sh`](https://github.com/AIQC-Hub/ctddump/blob/main/scripts/clean_data.sh).
