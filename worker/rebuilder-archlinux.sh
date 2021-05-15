#!/bin/sh
set -xe
repro -o "${REBUILDERD_OUTDIR}" -- "${1}"
