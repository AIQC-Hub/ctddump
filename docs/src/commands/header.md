# `header`

Extract the **metadata** of a single NetCDF file to a YAML file — dimensions,
variables, and global attributes. No observation data is read.

```
ctddump header <subcommand> <src_file> <target_file>
```

| Subcommand | Source |
|------------|--------|
| `nrt`  | NRT files |
| `cora` | CORA files |

## Examples

```bash
ctddump header nrt  input.nc output.yaml
ctddump header cora input.nc output.yaml
```

To extract metadata for a whole directory at once, use
[`batch header`](./batch.md); to combine many YAML files into one, use
[`concat header`](./concat.md).
