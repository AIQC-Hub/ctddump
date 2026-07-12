#!/usr/bin/env bash
#
# dedup_data.sh — de-duplicate the cleaned Parquet outputs of clean_data.sh with
# ctddump, for the Arctic, Baltic, and Mediterranean seas. Four steps run in
# order, each reading the previous step's output:
#
#   markdup   Mark duplicate profiles (adds an is_dup column) + write a dups TSV.
#   report    Summarise the marked Parquet (with duplicate counts) as TSV.
#   dedup     Remove duplicates, keeping the profile with the most observations.
#   report    Summarise the de-duplicated Parquet as TSV.
#
# Duplicates are decided by profile_timestamp (date only), longitude, and
# latitude (rounded to 3 decimals) — ctddump's defaults. The stages chain
# through sub-directories of $OUT/dedup:
#   $OUT/clean/filter -> dedup/markdup -> dedup/dedup
# and reports land in $OUT/report/dedup/{markdup,dedup}/.
#
# Usage:
#   scripts/dedup_data.sh [options] [command] [region ...]
#
# Commands:
#   markdup     Mark duplicate profiles.
#   report      Summarise the marked and/or de-duplicated Parquet as TSV.
#   dedup       Remove duplicate profiles.
#   all         markdup, report (marked), dedup, then report (deduped).  (default)
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -o, --out DIR   root for the clean_data.sh outputs and the de-duplicated
#                   outputs (default: output)
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# Requires: ctddump on PATH, and clean_data.sh's cleaned Parquet in
# <out>/clean/filter.

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
OUT=output
ASSUME_YES=0

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)  OUT="${2:?--out requires a directory}"; shift 2 ;;
    --out=*)   OUT="${1#*=}"; shift ;;
    -y|--yes)  ASSUME_YES=1; shift ;;
    -h|--help) usage; exit 0 ;;
    --)        shift; ARGS+=("$@"); break ;;
    -*)        echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)         ARGS+=("$1"); shift ;;
  esac
done

# Stage directories (each step reads the previous one).
SRC_DIR="$OUT/clean/filter"          # clean_data.sh cleaned Parquet (input)
MARK_DIR="$OUT/dedup/markdup"        # marked Parquet + dups TSV
DEDUP_DIR="$OUT/dedup/dedup"         # final de-duplicated data
REPORT_MARK_DIR="$OUT/report/dedup/markdup"
REPORT_DEDUP_DIR="$OUT/report/dedup/dedup"

REGIONS=(arctic baltic mediterranean)

# Merged-file stems produced by prepare_data.sh / carried through clean_data.sh.
stems_for() {  # <region>
  case "$1" in
    arctic)        echo nrt_ar_ar nrt_ar_gl cora_ar ;;
    baltic)        echo nrt_bo_bo cora_bo ;;
    mediterranean) echo nrt_mo_mo nrt_mo_gl cora_mo ;;
  esac
}

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so the currently running process
# is visible.

log() { printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$*" >&2; }

# Print the resolved configuration, then ask for confirmation unless -y/--yes was
# given. In a non-interactive shell without -y there is nothing to read, so abort
# with a hint rather than hang.
show_config() {  # <cmd> <region...>
  local cmd="$1"; shift
  {
    echo "Configuration:"
    echo "  command : $cmd"
    echo "  regions : $*"
    echo "  out     : $OUT"
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

# ---- Reusable per-region steps -------------------------------------------
# Each creates its output location so a fresh run works from scratch.

markdup_region() {  # <region>
  mkdir -p "$MARK_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "markdup $1/$s"
    ctddump markdup "$SRC_DIR/$s.parquet" "$MARK_DIR/$s.parquet" "$MARK_DIR/$s.dups.tsv"
  done
}

dedup_region() {  # <region>
  mkdir -p "$DEDUP_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "dedup $1/$s"
    ctddump dedup "$MARK_DIR/$s.parquet" "$DEDUP_DIR/$s.parquet"
  done
}

report_marked_region() {  # <region>
  mkdir -p "$REPORT_MARK_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "report (marked) $1/$s"
    ctddump report parquet --level platform "$MARK_DIR/$s.parquet" "$REPORT_MARK_DIR/$s.parquet.tsv"
  done
}

report_deduped_region() {  # <region>
  mkdir -p "$REPORT_DEDUP_DIR"
  local s
  for s in $(stems_for "$1"); do
    log "report (deduped) $1/$s"
    ctddump report parquet --level platform "$DEDUP_DIR/$s.parquet" "$REPORT_DEDUP_DIR/$s.parquet.tsv"
  done
}

# The standalone `report` command summarises whichever stages exist.
report_region() {  # <region>
  [[ -d "$MARK_DIR" ]] && report_marked_region "$1"
  [[ -d "$DEDUP_DIR" ]] && report_deduped_region "$1"
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

main() {
  local cmd="${1:-all}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    markdup|dedup|report|all) ;;
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

  for r in "${regions[@]}"; do
    log "===== $cmd: $r ====="
    case "$cmd" in
      markdup) "markdup_region" "$r" ;;
      dedup)   "dedup_region" "$r" ;;
      report)  "report_region" "$r" ;;
      all)     "markdup_region" "$r"; "report_marked_region" "$r"; "dedup_region" "$r"; "report_deduped_region" "$r" ;;
    esac
  done
  log "done."
}

main ${ARGS[@]+"${ARGS[@]}"}
