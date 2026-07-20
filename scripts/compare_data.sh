#!/usr/bin/env bash
#
# compare_data.sh: cross-compare the de-duplicated Parquet products of
# dedup_data.sh with ctddump, for the Arctic, Baltic, and Mediterranean seas.
# Three comparisons run per region, each writing a two-way coverage summary:
#
#   nrt-cora   Regional NRT (AR/BO/MO) vs CORA.
#   gl-cora    NRT Global (GL) subset  vs CORA.
#   nrt-gl     Regional NRT (AR/BO/MO) vs NRT Global (GL) subset.
#
# Each product covers the same waters, so the summary says how many platforms and
# profiles they share and, for the matched profiles, whether they carry the same
# number of observations. The keys are ctddump's defaults: platform_code,
# profile_time (date only), and longitude/latitude rounded to 3 decimals.
#
# The comparisons read the final de-duplicated Parquet under $OUT/dedup/dedup
# (override with --src) and reports land in $REPORT/compare/:
#   <code>_nrt_vs_cora, <code>_gl_vs_cora, <code>_nrt_vs_gl
# where <code> is ar / bo / mo.
#
# Usage:
#   scripts/compare_data.sh [options] [command] [region ...]
#
# Commands:
#   nrt-cora    Regional NRT vs CORA.
#   gl-cora     NRT Global vs CORA.
#   nrt-gl      Regional NRT vs NRT Global.
#   all         All three comparisons.  (default)
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -o, --out DIR   root for the dedup_data.sh outputs (default: output)
#   --src DIR       directory holding the input Parquet files, overriding the
#                   default of <out>/dedup/dedup
#   -r, --report DIR  root for the summary reports (default: report)
#   --format FMT    ctddump compare output format: tsv (default), text, or json.
#                   Sets the report file extension (tsv / txt / json).
#   --no-platform-key  match on time and position only, ignoring platform_code
#                   (finds the same cast filed under two different codes).
#   --chunk-rows N  streaming chunk size in rows for ctddump; lower uses less
#                   memory but reads more Parquet row groups. Exported as
#                   CTDDUMP_CHUNK_ROWS   (default: ctddump's built-in 1,000,000)
#   --by-region     Parallelise per region instead of per comparison (coarser:
#                   one worker per region, its comparisons run in order).
#   --sequential    Run everything one comparison at a time (no parallelism).
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# By default the selected comparisons are run in parallel, one worker each.
# Requires ctddump on PATH, and dedup_data.sh's de-duplicated Parquet in
# <out>/dedup/dedup (or the directory given with --src).

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
OUT=output
SRC_DIR=        # empty → derived from $OUT below (--src overrides)
REPORT=report
FORMAT=tsv      # tsv | text | json  (--format)
NO_PLATFORM_KEY=0
CHUNK_ROWS=     # empty → ctddump's built-in default (see CTDDUMP_CHUNK_ROWS)
ASSUME_YES=0
PARALLEL=comparison   # comparison | region | none

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)     OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)      OUT="${1#*=}"; shift ;;
    --src)        SRC_DIR="${2:?--src requires a directory}"; shift 2 ;;
    --src=*)      SRC_DIR="${1#*=}"; shift ;;
    -r|--report)  REPORT="${2:?--report requires a directory}"; shift 2 ;;
    --report=*)   REPORT="${1#*=}"; shift ;;
    --format)     FORMAT="${2:?--format requires a value}"; shift 2 ;;
    --format=*)   FORMAT="${1#*=}"; shift ;;
    --no-platform-key) NO_PLATFORM_KEY=1; shift ;;
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

case "$FORMAT" in
  tsv|text|json) ;;
  *) echo "Unknown --format: $FORMAT (use tsv, text, or json)" >&2; exit 1 ;;
esac
# Report file extension follows the format (text writes a .txt file).
case "$FORMAT" in
  tsv)  EXT=tsv ;;
  text) EXT=txt ;;
  json) EXT=json ;;
esac

# Input directory: --src wins, otherwise the dedup_data.sh final stage.
[[ -z "$SRC_DIR" ]] && SRC_DIR="$OUT/dedup/dedup"
REPORT_DIR="$REPORT/compare"

# A --chunk-rows value is passed to every ctddump child via the env var it reads
# (CTDDUMP_CHUNK_ROWS); leaving it unset keeps ctddump's built-in default.
[[ -n "$CHUNK_ROWS" ]] && export CTDDUMP_CHUNK_ROWS="$CHUNK_ROWS"

REGIONS=(arctic baltic mediterranean)
# The comparison kinds, in run order.
KINDS=(nrt-cora gl-cora nrt-gl)

# Region → two-letter code used in the merged-file stems.
code_for() {  # <region>
  case "$1" in
    arctic)        echo ar ;;
    baltic)        echo bo ;;
    mediterranean) echo mo ;;
  esac
}

# The three product stems for a region (as produced by convert_data.sh and
# carried through clean_data.sh / dedup_data.sh).
nrt_stem()  { local c; c=$(code_for "$1"); echo "nrt_${c}_${c}"; }  # regional NRT
gl_stem()   { local c; c=$(code_for "$1"); echo "nrt_${c}_gl"; }    # NRT Global subset
cora_stem() { local c; c=$(code_for "$1"); echo "cora_${c}"; }      # CORA

# ---- Logging -------------------------------------------------------------
# Announce each comparison (timestamped, to stderr) so the running unit is
# visible. Each parallel region worker sets REGION so its lines are tagged.
log() {
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >&2
}

# ---- Confirmation --------------------------------------------------------
show_config() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a rs=("$@")
  local nunits=0 r k
  for r in "${rs[@]}"; do
    for k in $(kinds_for "$cmd"); do nunits=$((nunits + 1)); done
  done
  local mode
  case "$PARALLEL" in
    none)       mode="sequential" ;;
    region)     [[ ${#rs[@]} -gt 1 ]] && mode="parallel (per region)" || mode="sequential" ;;
    comparison) [[ $nunits -gt 1 ]] && mode="parallel (per comparison)" || mode="sequential" ;;
  esac
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : ${rs[*]}"
    echo "  compares: $nunits"
    echo "  src     : $SRC_DIR"
    echo "  report  : $REPORT_DIR"
    echo "  format  : $FORMAT"
    echo "  platform key: $([[ "$NO_PLATFORM_KEY" == 1 ]] && echo off || echo on)"
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

# ---- Per-comparison step -------------------------------------------------
# Resolve a comparison kind for a region into its two input stems and output
# name. Echoes "<a_stem> <b_stem> <out_name>". The a/b order matches the command
# name (a vs b); ctddump reports both directions regardless.
resolve_kind() {  # <region> <kind>
  local r="$1" k="$2" c; c=$(code_for "$r")
  case "$k" in
    nrt-cora) echo "$(nrt_stem "$r") $(cora_stem "$r") ${c}_nrt_vs_cora" ;;
    gl-cora)  echo "$(gl_stem "$r")  $(cora_stem "$r") ${c}_gl_vs_cora" ;;
    nrt-gl)   echo "$(nrt_stem "$r") $(gl_stem "$r")   ${c}_nrt_vs_gl" ;;
  esac
}

# Run one comparison. A unit whose input is missing is skipped (with a note), not
# an error, so an unavailable product such as a region's Global (GL) file does not
# fail the run.
do_compare() {  # <region> <kind>
  local r="$1" k="$2"
  local a b name
  read -r a b name < <(resolve_kind "$r" "$k")
  local fa="$SRC_DIR/$a.parquet" fb="$SRC_DIR/$b.parquet"
  [[ -f "$fa" ]] || { log "skip $name (missing $a)"; return 0; }
  [[ -f "$fb" ]] || { log "skip $name (missing $b)"; return 0; }
  mkdir -p "$REPORT_DIR"; log "compare $name ($a vs $b)"
  local -a flags=(--format "$FORMAT")
  [[ "$NO_PLATFORM_KEY" == 1 ]] && flags+=(--no-platform-key)
  ctddump compare "${flags[@]}" "$fa" "$fb" "$REPORT_DIR/$name.$EXT"
}

# The comparison kinds selected by a command ("all" → every kind).
kinds_for() {  # <cmd>
  if [[ "$1" == all ]]; then printf '%s\n' "${KINDS[@]}"; else echo "$1"; fi
}

# Run <cmd>'s comparisons for one region, in order.
run_region() {  # <cmd> <region>
  local cmd="$1" r="$2" k
  for k in $(kinds_for "$cmd"); do do_compare "$r" "$k"; done
}

# ---- Dispatch ------------------------------------------------------------
is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

# Run <cmd> across the selected regions, honoring $PARALLEL:
#   comparison (default): one background worker per comparison (finest)
#   region              : one background worker per region (its comparisons run in order)
#   none                : everything sequentially, on the main shell
# Worker failures are collected; exit is non-zero if any unit failed.
run_all() {  # <cmd> <region...>
  local cmd="$1"; shift
  local -a regions=("$@")

  # Build the (region, kind) unit list.
  local -a ur=() uk=()
  local r k
  for r in "${regions[@]}"; do
    for k in $(kinds_for "$cmd"); do ur+=("$r"); uk+=("$k"); done
  done

  # Sequential, or nothing worth parallelizing.
  if [[ "$PARALLEL" == none || ${#ur[@]} -le 1 ]]; then
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
  else  # comparison (default)
    log "starting ${#ur[@]} comparisons in parallel (--by-region or --sequential to change)"
    for i in "${!ur[@]}"; do
      ( REGION="${ur[$i]}"; do_compare "${ur[$i]}" "${uk[$i]}" ) </dev/null &
      pids+=("$!"); labels+=("${ur[$i]}/${uk[$i]}")
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
  local cmd="${1:-all}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    nrt-cora|gl-cora|nrt-gl|all) ;;
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
