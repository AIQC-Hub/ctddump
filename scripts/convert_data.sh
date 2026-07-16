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
#   -t, --threads N   worker threads for ctddump           (default: 2)
#   -s, --src DIR     root of the downloaded NetCDF tree   (default: source)
#   -o, --out DIR     root for the generated data outputs  (default: output)
#   -r, --report DIR  root for the summary reports         (default: report)
#   --chunk-rows N    streaming chunk size in rows for ctddump; lower uses less
#                     memory but writes more Parquet row groups. Exported as
#                     CTDDUMP_CHUNK_ROWS   (default: ctddump's built-in 1,000,000)
#   --by-region     Parallelise per region instead of per unit (coarser: one
#                   worker per region, its three products processed in order).
#   --sequential    Process everything one unit at a time (no parallelism).
#   -y, --yes         Skip the confirmation prompt and start immediately.
#   -h, --help        Show this help.
#
# By default each (region, product) unit — the regional NRT, the Global (GL), and
# the CORA product of every selected region — runs in parallel, one worker per
# unit. Each ctddump call within a unit still uses -t/--threads worker threads.
#
# Requires: ctddump on PATH, and the source NetCDF in <src> (see download_data.sh).

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
THREADS=2
SRC=source
OUT=output
REPORT=report
CHUNK_ROWS=      # empty → ctddump's built-in default (see CTDDUMP_CHUNK_ROWS)
ASSUME_YES=0
PARALLEL=unit   # unit | region | none

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
    -r|--report)  REPORT="${2:?--report requires a directory}"; shift 2 ;;
    --report=*)   REPORT="${1#*=}"; shift ;;
    --chunk-rows) CHUNK_ROWS="${2:?--chunk-rows requires a value}"; shift 2 ;;
    --chunk-rows=*) CHUNK_ROWS="${1#*=}"; shift ;;
    --by-region)  PARALLEL=region; shift ;;
    --sequential) PARALLEL=none; shift ;;
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

# Each parallel worker sets REGION so its lines are tagged "[region]".
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
  local -a rs=("$@")
  local nunits=$(( ${#rs[@]} * ${#PRODUCTS[@]} ))
  local mode
  case "$PARALLEL" in
    none)   mode="sequential" ;;
    region) [[ ${#rs[@]} -gt 1 ]] && mode="parallel (per region)" || mode="sequential" ;;
    unit)   [[ $nunits -gt 1 ]] && mode="parallel (per unit)" || mode="sequential" ;;
  esac
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : ${rs[*]}"
    echo "  units   : $nunits"
    echo "  threads : $THREADS"
    echo "  src     : $SRC"
    echo "  out     : $OUT"
    echo "  report  : $REPORT"
    echo "  chunk   : ${CHUNK_ROWS:-default}"
    echo "  mode    : $mode"
    echo "Run with -h/--help to see all options."
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

# ---- Per-unit pipeline ---------------------------------------------------
# The work of a region splits into three independent products (units), each with
# its own output sub-directory, so they can run in parallel:
#   regional  the region's own NRT product (AR / BO / MO)
#   gl        the Global (GL) NRT product
#   cora      the CORA product
PRODUCTS=(regional gl cora)

# Per-region constants: "<code> <nrt_dir> <cora_subdir>". Note the CORA sub-dir
# for the Mediterranean is "mediterrane" (as published), not "mediterranean".
region_meta() {  # <region>
  case "$1" in
    arctic)        echo "ar $NRT_AR_DIR arctic" ;;
    baltic)        echo "bo $NRT_BO_DIR baltic" ;;
    mediterranean) echo "mo $NRT_MO_DIR mediterrane" ;;
  esac
}

# Convert + merge Parquet and export + merge headers for one (region, product).
# Copernicus does not yet publish the Global (GL) product for the Baltic, so the
# baltic/gl unit currently matches no files: the tools write nothing and the
# downstream scripts skip the missing nrt_bo_gl outputs, activating automatically
# once the GL data becomes available.
process_unit() {  # <region> <product: regional|gl|cora>
  local r="$1" prod="$2" rc nrtdir corasub
  read -r rc nrtdir corasub <<<"$(region_meta "$r")"
  local P="$OUT/convert" H="$OUT/header"
  local nrt="$SRC/$nrtdir" cora="$SRC/$CORA/$corasub"
  case "$prod" in
    regional)
      local rcu; rcu=$(printf '%s' "$rc" | tr '[:lower:]' '[:upper:]')
      convert    "nrt_$rc"          "$SRC" "$P/$rc/$rc"
      merge      "$P/$rc/$rc"              "$P/nrt_${rc}_${rc}.parquet"
      header_nrt "${rcu}_PR_CT_*.nc" "$nrt" "$H/$rc/$rc"
      merge_hdr  "$H/$rc/$rc"              "$H/nrt_${rc}_${rc}.yaml"
      ;;
    gl)
      convert    nrt_gl          "$nrt" "$P/$rc/gl"
      merge      "$P/$rc/gl"             "$P/nrt_${rc}_gl.parquet"
      header_nrt "GL_PR_CT_*.nc" "$nrt" "$H/$rc/gl"
      merge_hdr  "$H/$rc/gl"             "$H/nrt_${rc}_gl.yaml"
      ;;
    cora)
      convert     cora          "$cora" "$P/$rc/cora"
      merge       "$P/$rc/cora"          "$P/cora_${rc}.parquet"
      header_cora "$cora"                "$H/$rc/cora"
      merge_hdr   "$H/$rc/cora"          "$H/cora_${rc}.yaml"
      ;;
  esac
}

# ---- Per-unit reports (summaries of the merged outputs) ------------------
# Parquet reports use the platform level. Parquet-data summaries land in
# $REPORT/convert/ and header (YAML) summaries in $REPORT/header/, each named
# after its source with a .parquet.tsv / .yaml.tsv suffix. (clean_data.sh writes
# its reports under $REPORT/clean/, dedup_data.sh under $REPORT/dedup/.)

report_unit() {  # <region> <product: regional|gl|cora>
  local r="$1" prod="$2" rc _nrtdir _corasub
  read -r rc _nrtdir _corasub <<<"$(region_meta "$r")"
  local P="$OUT/convert" H="$OUT/header" RC="$REPORT/convert" RH="$REPORT/header"
  local stem
  case "$prod" in
    regional) stem="nrt_${rc}_${rc}" ;;
    gl)       stem="nrt_${rc}_gl" ;;
    cora)     stem="cora_${rc}" ;;
  esac
  report_parquet "$P/$stem.parquet" "$RC/$stem.parquet.tsv"
  report_yaml    "$H/$stem.yaml"    "$RH/$stem.yaml.tsv"
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

# Run <cmd> for one (region, product) unit.
run_unit() {  # <cmd> <region> <product>
  local cmd="$1" r="$2" p="$3"
  case "$cmd" in
    process) process_unit "$r" "$p" ;;
    report)  report_unit "$r" "$p" ;;
    all)     process_unit "$r" "$p"; report_unit "$r" "$p" ;;
  esac
}

# Run <cmd> for every product of <region>, in order.
run_region() {  # <cmd> <region>
  local cmd="$1" r="$2" p
  for p in "${PRODUCTS[@]}"; do run_unit "$cmd" "$r" "$p"; done
}

# Run <cmd> across all units of the selected regions, honoring $PARALLEL:
#   unit   (default) — one background worker per (region, product) unit
#   region           — one background worker per region (its products run in order)
#   none             — everything sequentially, on the main shell
# Each worker detaches stdin and tags its log lines with its region. Worker
# failures are collected; exit is non-zero if any unit failed.
run_all() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a regions=("$@")

  # Build the (region, product) unit list.
  local -a jr=() jp=()
  local r p
  for r in "${regions[@]}"; do
    for p in "${PRODUCTS[@]}"; do jr+=("$r"); jp+=("$p"); done
  done

  # Sequential, or nothing worth parallelizing.
  if [[ "$PARALLEL" == none || ${#jr[@]} -le 1 ]]; then
    for r in "${regions[@]}"; do
      log "===== $cmd: $r ====="
      run_region "$cmd" "$r"
    done
    return 0
  fi

  local -a pids=() labels=()
  local i
  if [[ "$PARALLEL" == region ]]; then
    log "starting ${#regions[@]} regions in parallel (--sequential to disable)"
    for r in "${regions[@]}"; do
      ( REGION="$r"; run_region "$cmd" "$r" ) </dev/null &
      pids+=("$!"); labels+=("$r")
    done
  else  # unit (default)
    log "starting ${#jr[@]} units in parallel (--by-region or --sequential to change)"
    for i in "${!jr[@]}"; do
      ( REGION="${jr[$i]}"; run_unit "$cmd" "${jr[$i]}" "${jp[$i]}" ) </dev/null &
      pids+=("$!"); labels+=("${jr[$i]}/${jp[$i]}")
    done
  fi

  local fail=0
  for i in "${!pids[@]}"; do
    if ! wait "${pids[$i]}"; then
      log "'${labels[$i]}' FAILED"; fail=1
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

  run_all "$cmd" "${regions[@]}" || { log "one or more units failed."; return 1; }
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
