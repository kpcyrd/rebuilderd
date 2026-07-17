#!/bin/bash
# rebuilderd backend for OpenWrt firmware images, rebuilt from source.
#
# The from-source build is done by the openwrt-rebuilder tool
# (`openwrt-rebuilder firmware ...`): it checks out openwrt.git with FULL
# history, restores the release .config and pins the feeds from the published
# *.buildinfo, builds the whole target the way OpenWrt's buildbots do — directly
# in this worker container (no nested container) — and writes every produced
# artifact (plus build logs) into the --output dir for the worker to compare.
# openwrt-rebuilder takes plain CLI parameters, so this wrapper just derives the
# release + target/subtarget from the input URL and passes them as flags.
#
# Must run as a non-root user (OpenWrt refuses to build as root).
#
# Inputs (from the worker):
#   $1                    downloaded build input — content unused; identity only
#   REBUILDERD_OUTDIR     where to write rebuilt artifacts (the worker compares them)
#   REBUILDERD_INPUT_URL  a URL under .../targets/<target>/<subtarget>/ — release,
#                         target and subtarget are parsed from its path
#
# Env (optional):
#   OPENWRT_DL_DIR    persistent source download cache
#                     (default $HOME/.cache/rebuilderd-openwrt-dl)
#   OPENWRT_MAX_JOBS  cap on build parallelism (default 30)
#   TMPDIR            parent for the per-build scratch dir (default /tmp)
set -eu

: "${REBUILDERD_OUTDIR:?must be set}"
: "${REBUILDERD_INPUT_URL:?must be set — need a .../targets/<target>/<subtarget>/ URL}"

# --- derive release / target / subtarget from the input URL ---
url="${REBUILDERD_INPUT_URL%%[?#]*}"
case "$url" in
    */targets/*) ;;
    *) echo "no /targets/ segment in REBUILDERD_INPUT_URL: $url" >&2; exit 1 ;;
esac
after="${url#*/targets/}"
TARGET="${after%%/*}"
after="${after#*/}"
SUBTARGET="${after%%/*}"
case "$url" in
    */snapshots/targets/*) RELEASE="SNAPSHOT" ;;
    */releases/*/targets/*) tail="${url#*/releases/}"; RELEASE="${tail%%/*}" ;;
    *) echo "cannot derive release from REBUILDERD_INPUT_URL: $url" >&2; exit 1 ;;
esac
if [ -z "$TARGET" ] || [ -z "$SUBTARGET" ]; then
    echo "failed to parse target/subtarget from $url" >&2
    exit 1
fi
TS="$TARGET/$SUBTARGET"

DL_DIR="${OPENWRT_DL_DIR:-$HOME/.cache/rebuilderd-openwrt-dl}"
# Fresh scratch tree per build (reproducibility needs a clean checkout); removed after.
BUILD_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/rebuilderd-openwrt-image.XXXXXX")
trap 'rm -rf "$BUILD_ROOT"' EXIT
mkdir -p "$DL_DIR"

# Cap build parallelism. OpenWrt builds the host LLVM/BPF toolchain from source
# and its largest clang translation units need ~2 GB RAM each, so -j$(nproc) can
# OOM on a many-core box (nproc reports the *host* core count inside a
# container). 30 stays within memory here — override with OPENWRT_MAX_JOBS.
MAX_JOBS="${OPENWRT_MAX_JOBS:-30}"
CPUS=$(nproc)
if [ "$CPUS" -lt "$MAX_JOBS" ]; then JOBS="$CPUS"; else JOBS="$MAX_JOBS"; fi

echo "=== openwrt firmware rebuild ==="
echo "release: $RELEASE"
echo "target:  $TS"
echo "jobs:    -j$JOBS (cpus=$CPUS, capped at $MAX_JOBS)"
echo "outdir:  $REBUILDERD_OUTDIR"

# Not exec'd: keep the shell alive so the EXIT trap removes the scratch tree.
openwrt-rebuilder firmware \
    --target "$TS" \
    --release "$RELEASE" \
    --output "$REBUILDERD_OUTDIR" \
    --jobs "$JOBS" \
    --build-dir "$BUILD_ROOT/src" \
    --dl-dir "$DL_DIR"
