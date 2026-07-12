#!/usr/bin/env bash
#
# download_data.sh — download CTD source NetCDF from the Copernicus Marine
# Service into a local tree, for the Arctic, Baltic, and Mediterranean seas.
# Convert the downloaded files to Parquet with convert_data.sh.
#
# Usage:
#   scripts/download_data.sh [options] [command] [region ...]
#
# Commands:
#   login       Log in to the Copernicus Marine Toolbox (once, interactively).
#   download    Download the source NetCDF files.  (default)
#   help        Show this help.
#
# Regions:  arctic  baltic  mediterranean   (default: all three; "all" also works)
#
# Options (may appear anywhere on the command line):
#   -s, --src DIR   directory to download the NetCDF tree into  (default: input)
#   --sequential    Download regions one at a time (default: selected regions
#                   download in parallel when more than one is chosen).
#   -y, --yes       Skip the confirmation prompt and start immediately.
#   -h, --help      Show this help.
#
# Requires: copernicusmarine on PATH. A free Copernicus Marine account is needed:
#   https://help.marine.copernicus.eu/en/collections/9080063-copernicus-marine-toolbox

set -euo pipefail

usage() { awk 'NR<3 {next} /^#/ {sub(/^# ?/, ""); print; next} {exit}' "$0"; }

# ---- Configuration (defaults; override with the options below) -----------
SRC=input
ASSUME_YES=0
SEQUENTIAL=0

# ---- Parse options -------------------------------------------------------
# Options may appear anywhere; the remaining words are the command and regions.
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -s|--src)     SRC="${2:?--src requires a directory}"; shift 2 ;;
    --src=*)      SRC="${1#*=}"; shift ;;
    --sequential) SEQUENTIAL=1; shift ;;
    -y|--yes)     ASSUME_YES=1; shift ;;
    -h|--help)    usage; exit 0 ;;
    --)           shift; ARGS+=("$@"); break ;;
    -*)           echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)            ARGS+=("$1"); shift ;;
  esac
done

REGIONS=(arctic baltic mediterranean)

# ---- Logging -------------------------------------------------------------
# Announce each step (timestamped, to stderr) so the currently running process
# is visible. `log` prints a message; `run` logs the command then executes it.

# Each parallel region worker sets REGION so its lines are tagged "[region]".
log() {
  local p=""
  [[ -n "${REGION:-}" ]] && p="[$REGION] "
  printf '[%s] %s%s\n' "$(date '+%H:%M:%S')" "$p" "$*" >&2
}
run() { log "RUN: $*"; "$@"; }

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
    echo "  src     : $SRC"
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

# ---- Download ------------------------------------------------------------
login() { copernicusmarine login; }

# `copernicusmarine get` writes each product's directory under the current
# directory, so download into $SRC by cd-ing there (created as needed). The cd
# is scoped to a subshell so parallel/sequential regions don't interfere.

download_arctic() {
  mkdir -p "$SRC"
  ( cd "$SRC"
    run copernicusmarine get -i cmems_obs-ins_arc_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
    run copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "arctic/*/*_PR_CT.nc" )
}

download_baltic() {
  mkdir -p "$SRC"
  ( cd "$SRC"
    run copernicusmarine get -i cmems_obs-ins_bal_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
    run copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "baltic/*/*_PR_CT.nc" )
}

download_mediterranean() {
  mkdir -p "$SRC"
  ( cd "$SRC"
    run copernicusmarine get -i cmems_obs-ins_med_phybgcwav_mynrt_na_irr --dataset-part "history" --filter "*/CT/*"
    run copernicusmarine get -i cmems_obs-ins_glo_phy-temp-sal_my_cora_irr --filter "mediterrane/*/*_PR_CT.nc" )
}

# ---- Dispatch ------------------------------------------------------------

is_region() {
  local r
  for r in "${REGIONS[@]}"; do [[ "$r" == "$1" ]] && return 0; done
  return 1
}

# Run one region's download for <cmd>.
run_region() {  # <cmd> <region>
  local cmd="$1" r="$2"
  case "$cmd" in
    download) "download_$r" ;;
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
  local cmd="${1:-download}"
  [[ $# -gt 0 ]] && shift

  case "$cmd" in
    -h|--help|help) usage; return 0 ;;
    login) login; return 0 ;;
    download) ;;
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
