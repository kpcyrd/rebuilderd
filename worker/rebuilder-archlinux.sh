#!/bin/sh
set -xe
OUTDIR="${REBUILDERD_OUTDIR}" repro -- "${1}"
