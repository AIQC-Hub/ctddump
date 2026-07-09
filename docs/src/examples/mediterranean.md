# Mediterranean Sea

An end-to-end workflow for the Mediterranean Sea: download the source files,
convert them to Parquet, merge them, and export the metadata.

## Prerequisites

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

## 1. Download the data

```shell
# NRT — Mediterranean (MO) and Global (GL)
copernicusmarine get -i cmems_obs-ins_med_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"

# CORA — Mediterranean
copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "mediterrane/*/*_PR_CT.nc"
```

## 2. Convert NetCDF to Parquet

```shell
# NRT MO
ctddump batch convert nrt_mo --threads 10 --output ../process_data/ctddump/parquet/mo/mo ../source_data/ctddump/netcdf

# NRT GL
ctddump batch convert nrt_gl --threads 10 --output ../process_data/ctddump/parquet/mo/gl ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch convert cora --threads 10 --output ../process_data/ctddump/parquet/mo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

## 3. Merge the Parquet files

```shell
# NRT MO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/mo ../process_data/ctddump/parquet/nrt_mo_mo.parquet

# NRT GL
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/gl ../process_data/ctddump/parquet/nrt_mo_gl.parquet

# CORA MO
ctddump concat convert --threads 10 ../process_data/ctddump/parquet/mo/cora ../process_data/ctddump/parquet/cora_mo.parquet
```

## 4. Export the metadata (headers)

```shell
# NRT MO
ctddump batch header nrt --threads 10 --pattern "MO_PR_CT_*.nc" --output ../process_data/ctddump/header/mo/mo ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# NRT GL
ctddump batch header nrt --threads 10 --pattern "GL_PR_CT_*.nc" --output ../process_data/ctddump/header/mo/gl ../source_data/ctddump/netcdf/INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035

# CORA MO
ctddump batch header cora --threads 10 --output ../process_data/ctddump/header/mo/cora ../source_data/ctddump/netcdf/INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511/mediterrane
```

## 5. Merge the header files

```shell
# NRT MO
ctddump concat header ../process_data/ctddump/header/mo/mo ../process_data/ctddump/header/nrt_mo_mo.yaml

# NRT GL
ctddump concat header ../process_data/ctddump/header/mo/gl ../process_data/ctddump/header/nrt_mo_gl.yaml

# CORA MO
ctddump concat header ../process_data/ctddump/header/mo/cora ../process_data/ctddump/header/cora_mo.yaml
```

## 6. Summarise the results

Write a global-level summary of each merged Parquet file and a per-file summary
of each merged header YAML (as TSV).

```shell
# NRT MO
ctddump report parquet --level global ../process_data/ctddump/parquet/nrt_mo_mo.parquet ../process_data/ctddump/report/nrt_mo_mo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_mo_mo.yaml ../process_data/ctddump/report/nrt_mo_mo.yaml.tsv

# NRT GL
ctddump report parquet --level global ../process_data/ctddump/parquet/nrt_mo_gl.parquet ../process_data/ctddump/report/nrt_mo_gl.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/nrt_mo_gl.yaml ../process_data/ctddump/report/nrt_mo_gl.yaml.tsv

# CORA MO
ctddump report parquet --level global ../process_data/ctddump/parquet/cora_mo.parquet ../process_data/ctddump/report/cora_mo.parquet.tsv
ctddump report yaml ../process_data/ctddump/header/cora_mo.yaml ../process_data/ctddump/report/cora_mo.yaml.tsv
```
