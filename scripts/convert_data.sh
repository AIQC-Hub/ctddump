#!/usr/bin/env bash
#
# convert_data.sh — convert the CTD source NetCDF (fetched by download_data.sh)
# to Parquet (data) and YAML (metadata) with ctddump, for the Arctic, Baltic,
# and Mediterranean seas: batch-convert, merge, export + merge headers, and
# summarise.
#
# Usage:
#   scripts/convert_data.sh [options] [command] [region ...]
#
# Commands:
#   process     Convert + merge Parquet, export + merge headers.  (default)
#   report      Summarise the merged Parquet/YAML outputs as TSV.
#   all         process, then report.
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -t, --threads N   worker threads for ctddump          (default: 10)
#   -s, --src DIR     root of the downloaded NetCDF tree  (default: input)
#   -o, --out DIR     root for the generated outputs      (default: output)
#   --chunk-rows N    streaming chunk size in rows for ctddump; lower uses less
#                     memory but writes more Parquet row groups. Exported as
#                     CTDDUMP_CHUNK_ROWS   (default: ctddump's built-in 1,000,000)
#   --sequential      Process regions one at a time (default: selected regions
#                     run in parallel when more than one is chosen).
#   -y, --yes         Skip the confirmation prompt and start immediately.
#   -h, --help        Show this help.
#
# Requires: ctddump on PATH, and the source NetCDF in <src> (see download_data.sh).

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
THREADS=10
SRC=input
OUT=output
CHUNK_ROWS=      # empty → ctddump's built-in default (see CTDDUMP_CHUNK_ROWS)
ASSUME_YES=0
SEQUENTIAL=0

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -t|--threads) THREADS="${2:?--threads requires a value}"; shift 2 ;;
    --threads=*)  THREADS="${1#*=}"; shift ;;
    -s|--src)     SRC="${2:?--src requires a directory}"; shift 2 ;;
    --src=*)      SRC="${1#*=}"; shift ;;
    -o|--out)     OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)      OUT="${1#*=}"; shift ;;
    --chunk-rows) CHUNK_ROWS="${2:?--chunk-rows requires a value}"; shift 2 ;;
    --chunk-rows=*) CHUNK_ROWS="${1#*=}"; shift ;;
    --sequential) SEQUENTIAL=1; shift ;;
    -y|--yes)     ASSUME_YES=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; ARGS+=("$@"); break ;;
    -*)           echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)            ARGS+=("$1"); shift ;;
  esac
done

# A --chunk-rows value is passed to every ctddump child process via the env var
# it reads (CTDDUMP_CHUNK_ROWS); leaving it unset keeps ctddump's built-in default.
[[ -n "$CHUNK_ROWS" ]] && export CTDDUMP_CHUNK_ROWS="$CHUNK_ROWS"

# Copernicus product directories under $SRC.
NRT_AR_DIR="INSITU_ARC_PHYBGCWAV_DISCRETE_MYNRT_013_031"
NRT_BO_DIR="INSITU_BAL_PHYBGCWAV_DISCRETE_MYNRT_013_032"
NRT_MO_DIR="INSITU_MED_PHYBGCWAV_DISCRETE_MYNRT_013_035"
CORA="INSITU_GLO_PHY_TS_DISCRETE_MY_013_001/cmems_obs-ins_glo_phy-temp-sal_my_cora_irr_202511"

REGIONS=(arctic baltic mediterranean)

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so the currently running process
# is visible.

# Each parallel region worker sets REGION so its lines are tagged "[region]".
log() {
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >&2
}

# Print the resolved configuration, then ask for confirmation unless -y/--yes was
# given. In a non-interactive shell without -y there is nothing to read, so abort
# with a hint rather than hang.
show_config() {  # <cmd> <region...>
  local cmd="$1"; shift
  local mode="sequential"
  [[ "$SEQUENTIAL" != 1 && $# -gt 1 ]] && mode="parallel (per region)"
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : $*"
    echo "  threads : $THREADS"
    echo "  src     : $SRC"
    echo "  out     : $OUT"
    echo "  chunk   : ${CHUNK_ROWS:-default}"
    echo "  mode    : $mode"
  } >&2
}

confirm() {
  [[ "$ASSUME_YES" == 1 ]] && return 0
  if [[ ! -t 0 ]]; then
    log "non-interactive shell: pass -y/--yes to proceed without confirmation."
    return 1
  fi
  local reply
  read -r -p "Proceed? [y/N] " reply
  [[ "$reply" == [yY] || "$reply" == [yY][eE][sS] ]]
}

# ---- Reusable ctddump steps ----------------------------------------------
# Each creates its output location so a fresh run works from scratch.

convert() {  # <format> <src_dir> <out_dir>
  mkdir -p "$3"
  log "convert $1: $2 -> $3"
  ctddump batch convert "$1" --threads "$THREADS" --output "$3" "$2"
}

merge() {  # <src_dir> <out_file>
  mkdir -p "$(dirname "$2")"
  log "merge: $1 -> $2"
  ctddump concat convert --threads "$THREADS" "$1" "$2"
}

header_nrt() {  # <pattern> <src_dir> <out_dir>
  mkdir -p "$3"
  log "header nrt ($1): $2 -> $3"
  ctddump batch header nrt --threads "$THREADS" --pattern "$1" --output "$3" "$2"
}

header_cora() {  # <src_dir> <out_dir>
  mkdir -p "$2"
  log "header cora: $1 -> $2"
  ctddump batch header cora --threads "$THREADS" --output "$2" "$1"
}

merge_hdr() {  # <src_dir> <out_file>
  mkdir -p "$(dirname "$2")"
  log "merge header: $1 -> $2"
  ctddump concat header "$1" "$2"
}

report_parquet() {  # <src_file> <out_tsv> -- platform-level summary of a merged Parquet
  [[ -f "$1" ]] || { log "skip report parquet (missing $1)"; return 0; }
  mkdir -p "$(dirname "$2")"
  log "report parquet: $1 -> $2"
  ctddump report parquet --level platform "$1" "$2"
}

report_yaml() {  # <src_file> <out_tsv> -- summary of a merged header YAML
  [[ -f "$1" ]] || { log "skip report yaml (missing $1)"; return 0; }
  mkdir -p "$(dirname "$2")"
  log "report yaml: $1 -> $2"
  ctddump report yaml "$1" "$2"
}

# ---- Per-region pipelines ------------------------------------------------

process_arctic() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_AR_DIR" cora="$SRC/$CORA/arctic"

  convert nrt_ar "$SRC"  "$P/ar/ar"
  convert nrt_gl "$nrt"  "$P/ar/gl"
  convert cora   "$cora" "$P/ar/cora"

  merge "$P/ar/ar"   "$P/nrt_ar_ar.parquet"
  merge "$P/ar/gl"   "$P/nrt_ar_gl.parquet"
  merge "$P/ar/cora" "$P/cora_ar.parquet"

  header_nrt "AR_PR_CT_*.nc" "$nrt"  "$H/ar/ar"
  header_nrt "GL_PR_CT_*.nc" "$nrt"  "$H/ar/gl"
  header_cora                "$cora" "$H/ar/cora"

  merge_hdr "$H/ar/ar"   "$H/nrt_ar_ar.yaml"
  merge_hdr "$H/ar/gl"   "$H/nrt_ar_gl.yaml"
  merge_hdr "$H/ar/cora" "$H/cora_ar.yaml"
}

# The Baltic workflow uses the regional NRT (BO), Global (GL), and CORA products.
# Copernicus does not yet publish the Global (GL) data for the Baltic, so the GL
# steps currently match no files: the tools report this and write nothing, and the
# downstream scripts skip the missing nrt_bo_gl outputs. They activate
# automatically once the GL data becomes available.
process_baltic() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_BO_DIR" cora="$SRC/$CORA/baltic"

  convert nrt_bo "$SRC"  "$P/bo/bo"
  convert nrt_gl "$nrt"  "$P/bo/gl"
  convert cora   "$cora" "$P/bo/cora"

  merge "$P/bo/bo"   "$P/nrt_bo_bo.parquet"
  merge "$P/bo/gl"   "$P/nrt_bo_gl.parquet"
  merge "$P/bo/cora" "$P/cora_bo.parquet"

  header_nrt "BO_PR_CT_*.nc" "$nrt"  "$H/bo/bo"
  header_nrt "GL_PR_CT_*.nc" "$nrt"  "$H/bo/gl"
  header_cora                "$cora" "$H/bo/cora"

  merge_hdr "$H/bo/bo"   "$H/nrt_bo_bo.yaml"
  merge_hdr "$H/bo/gl"   "$H/nrt_bo_gl.yaml"
  merge_hdr "$H/bo/cora" "$H/cora_bo.yaml"
}

process_mediterranean() {
  local P="$OUT/parquet" H="$OUT/header"
  local nrt="$SRC/$NRT_MO_DIR" cora="$SRC/$CORA/mediterrane"

  convert nrt_mo "$SRC"  "$P/mo/mo"
  convert nrt_gl "$nrt"  "$P/mo/gl"
  convert cora   "$cora" "$P/mo/cora"

  merge "$P/mo/mo"   "$P/nrt_mo_mo.parquet"
  merge "$P/mo/gl"   "$P/nrt_mo_gl.parquet"
  merge "$P/mo/cora" "$P/cora_mo.parquet"

  header_nrt "MO_PR_CT_*.nc" "$nrt"  "$H/mo/mo"
  header_nrt "GL_PR_CT_*.nc" "$nrt"  "$H/mo/gl"
  header_cora                "$cora" "$H/mo/cora"

  merge_hdr "$H/mo/mo"   "$H/nrt_mo_mo.yaml"
  merge_hdr "$H/mo/gl"   "$H/nrt_mo_gl.yaml"
  merge_hdr "$H/mo/cora" "$H/cora_mo.yaml"
}

# ---- Per-region reports (summaries of the merged outputs) ----------------
# Parquet reports use the platform level; each report lands in
# $OUT/report/convert/ named after its source, with a .parquet.tsv /
# .yaml.tsv suffix. (clean_data.sh writes its reports to $OUT/report/clean/.)

report_arctic() {
  local P="$OUT/parquet" H="$OUT/header" R="$OUT/report/convert"
  report_parquet "$P/nrt_ar_ar.parquet" "$R/nrt_ar_ar.parquet.tsv"
  report_parquet "$P/nrt_ar_gl.parquet" "$R/nrt_ar_gl.parquet.tsv"
  report_parquet "$P/cora_ar.parquet"   "$R/cora_ar.parquet.tsv"
  report_yaml    "$H/nrt_ar_ar.yaml"     "$R/nrt_ar_ar.yaml.tsv"
  report_yaml    "$H/nrt_ar_gl.yaml"     "$R/nrt_ar_gl.yaml.tsv"
  report_yaml    "$H/cora_ar.yaml"       "$R/cora_ar.yaml.tsv"
}

report_baltic() {
  local P="$OUT/parquet" H="$OUT/header" R="$OUT/report/convert"
  report_parquet "$P/nrt_bo_bo.parquet" "$R/nrt_bo_bo.parquet.tsv"
  report_parquet "$P/nrt_bo_gl.parquet" "$R/nrt_bo_gl.parquet.tsv"
  report_parquet "$P/cora_bo.parquet"   "$R/cora_bo.parquet.tsv"
  report_yaml    "$H/nrt_bo_bo.yaml"     "$R/nrt_bo_bo.yaml.tsv"
  report_yaml    "$H/nrt_bo_gl.yaml"     "$R/nrt_bo_gl.yaml.tsv"
  report_yaml    "$H/cora_bo.yaml"       "$R/cora_bo.yaml.tsv"
}

report_mediterranean() {
  local P="$OUT/parquet" H="$OUT/header" R="$OUT/report/convert"
  report_parquet "$P/nrt_mo_mo.parquet" "$R/nrt_mo_mo.parquet.tsv"
  report_parquet "$P/nrt_mo_gl.parquet" "$R/nrt_mo_gl.parquet.tsv"
  report_parquet "$P/cora_mo.parquet"   "$R/cora_mo.parquet.tsv"
  report_yaml    "$H/nrt_mo_mo.yaml"     "$R/nrt_mo_mo.yaml.tsv"
  report_yaml    "$H/nrt_mo_gl.yaml"     "$R/nrt_mo_gl.yaml.tsv"
  report_yaml    "$H/cora_mo.yaml"       "$R/cora_mo.yaml.tsv"
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

# Run one region's pipeline for <cmd>.
run_region() {  # <cmd> <region>
  local cmd="$1" r="$2"
  case "$cmd" in
    process) "process_$r" ;;
    report)  "report_$r" ;;
    all)     "process_$r"; "report_$r" ;;
  esac
}

# Run <cmd> for every region. Regions run in parallel (one background worker
# each, with stdin detached) unless --sequential is set or only one region is
# selected. Worker failures are collected and reported; exit is non-zero if any
# region failed.
run_regions() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a regions=("$@")

  if [[ "$SEQUENTIAL" == 1 || ${#regions[@]} -le 1 ]]; then
    local r
    for r in "${regions[@]}"; do
      log "===== $cmd: $r ====="
      run_region "$cmd" "$r"
    done
    return 0
  fi

  log "starting ${#regions[@]} regions in parallel (--sequential to disable)"
  local -a pids=() regs=()
  local r
  for r in "${regions[@]}"; do
    ( REGION="$r"; log "===== $cmd: $r ====="; run_region "$cmd" "$r" ) </dev/null &
    pids+=("$!"); regs+=("$r")
  done
  local fail=0 i
  for i in "${!pids[@]}"; do
    if ! wait "${pids[$i]}"; then
      log "region '${regs[$i]}' FAILED"; fail=1
    fi
  done
  return "$fail"
}

main() {
  local cmd="${1:-process}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    process|report|all) ;;
    *) echo "Unknown command: $cmd" >&2; usage; return 1 ;;
  esac

  # Remaining args are regions; default to all, and "all" is an alias.
  local -a regions=("$@")
  if [[ ${#regions[@]} -eq 0 || "${regions[0]}" == "all" ]]; then
    regions=("${REGIONS[@]}")
  fi
  for r in "${regions[@]}"; do
    is_region "$r" || { echo "Unknown region: $r" >&2; usage; return 1; }
  done

  show_config "$cmd" "${regions[@]}"
  confirm || { log "aborted."; return 1; }

  run_regions "$cmd" "${regions[@]}" || { log "one or more regions failed."; return 1; }
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
